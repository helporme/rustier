use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use compiler::{
    codegen::{TransportBackend, TransportBackendNode, TransportBackendWasm, WasmImport},
    db::RootDatabase,
    diag::Theme,
    ir::js::{JsFile, JsProgram},
};
use napi::{
    Error, Result,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
};
use napi_derive::napi;
use notify::{RecursiveMode, Watcher};
use salsa::Setter;

#[napi(object)]
pub struct BackendOptions {
    #[napi(js_name = "type")]
    pub kind: String,
    pub mem64: Option<bool>,
    pub loading: Option<String>,
}

#[napi(object)]
pub struct ProfileOptions {
    pub release: Option<bool>,
    pub backend: BackendOptions,
    pub crates: Option<HashMap<String, String>>,
}

#[napi(object)]
pub struct ServerOptions {
    pub root_dir: String,
    pub profiles: HashMap<String, ProfileOptions>,
}

#[napi(object)]
#[derive(Clone)]
pub struct BuildResult {
    pub diagnostics: Vec<String>,
    pub bin_ffi_path: Option<String>,
}

#[napi(object)]
pub struct DependencyChange {
    pub profile: String,
    pub error: Option<String>,
}

type ChangeCallback =
    ThreadsafeFunction<DependencyChange, (), DependencyChange, napi::Status, false, true>;

struct Profile {
    program: JsProgram,
    directory: PathBuf,
    release: bool,
    backend: TransportBackend,
    crates: BTreeMap<String, String>,
    revision: Arc<AtomicU64>,
    built: Option<(u64, BuildResult)>,
}

#[napi]
pub struct Server {
    db: RootDatabase,
    profiles: BTreeMap<String, Profile>,
    watchers: Vec<notify::RecommendedWatcher>,
    watch_callback: Option<Arc<ChangeCallback>>,
    watch_graph_dirty: Arc<AtomicBool>,
}

#[napi]
impl Server {
    /// Creates a server with independent programs and build settings for each profile.
    ///
    /// Crate paths are resolved relative to `root_dir`. Returns an error if the
    /// root, crate paths, profile names, or backend settings are invalid.
    #[napi(constructor)]
    pub fn new(options: ServerOptions) -> Result<Self> {
        let root = fs::canonicalize(&options.root_dir)
            .map_err(|error| Error::from_reason(format!("invalid project root: {error}")))?;
        let db = RootDatabase::default();
        let mut profiles = BTreeMap::new();

        for (name, options) in options.profiles {
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            {
                return Err(Error::from_reason(format!(
                    "invalid profile name: {name:?}"
                )));
            }
            let backend = match options.backend.kind.as_str() {
                "wasm" => TransportBackend::Wasm(TransportBackendWasm {
                    mem64: options.backend.mem64.unwrap_or(false),
                    import: match options.backend.loading.as_deref().unwrap_or("fetch") {
                        "fetch" => WasmImport::Fetch,
                        "node-js" => WasmImport::NodeJs,
                        "module" => WasmImport::Module,
                        mode => {
                            return Err(Error::from_reason(format!(
                                "unsupported Wasm loading mode: {mode}"
                            )));
                        }
                    },
                }),
                "node" => TransportBackend::Node(TransportBackendNode),
                backend => {
                    return Err(Error::from_reason(format!(
                        "unsupported backend: {backend}"
                    )));
                }
            };
            let crates = options
                .crates
                .unwrap_or_default()
                .into_iter()
                .map(|(name, path)| {
                    let path = fs::canonicalize(root.join(path)).map_err(|error| {
                        Error::from_reason(format!("invalid crate path for {name}: {error}"))
                    })?;
                    Ok((name, path.to_string_lossy().into_owned()))
                })
                .collect::<Result<_>>()?;

            profiles.insert(
                name.clone(),
                Profile {
                    program: JsProgram::new(&db, Vec::new()),
                    directory: root.join(".rustier/bridge").join(name),
                    release: options.release.unwrap_or(false),
                    backend,
                    crates,
                    revision: Arc::new(AtomicU64::new(0)),
                    built: None,
                },
            );
        }

        Ok(Self {
            db,
            profiles,
            watchers: Vec::new(),
            watch_callback: None,
            watch_graph_dirty: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Renders the profile's JavaScript runtime using `bin_file_path` to load its binary.
    ///
    /// Does not build the binary. Returns an error if the profile is unknown.
    #[napi]
    pub fn render_js_ffi(&self, profile: String, bin_file_path: String) -> Result<String> {
        let profile = self.profile(&profile)?;
        Ok(profile.program.runtime(&self.db).render_js_ffi(
            &self.db,
            &profile.backend,
            &bin_file_path,
        ))
    }

    /// Adds or replaces a module in the profile and replaces its Rust snippet tags
    /// with calls imported from `ffi_js_mod_path`.
    ///
    /// Changed contents invalidate the cached build. Returns an error for an
    /// unknown profile or JavaScript diagnostics; the supplied contents remain
    /// registered even when diagnostics prevent transformation.
    #[napi]
    pub fn transform_module(
        &mut self,
        profile: String,
        module_id: String,
        contents: String,
        ffi_js_mod_path: String,
    ) -> Result<String> {
        let state = self.profile(&profile)?;
        let program = state.program;
        let revision = state.revision.clone();
        let file = JsFile::new(&self.db, module_id.clone(), contents);
        let mut files = program.files(&self.db).clone();
        if !files.contains(&file) {
            match files
                .iter_mut()
                .find(|entry| entry.module_id(&self.db) == &module_id)
            {
                Some(entry) => *entry = file,
                None => files.push(file),
            }
            program.set_files(&mut self.db).to(files);
            revision.fetch_add(1, Ordering::SeqCst);
        }

        let diagnostics = file.diagnostics(&self.db);
        if !diagnostics.is_empty() {
            let messages = diagnostics
                .iter()
                .map(|diagnostic| diagnostic.render(Theme::UnicodeNoColor))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|error| Error::from_reason(error.to_string()))?;
            return Err(Error::from_reason(messages.join("\n")));
        }

        Ok(file
            .module(&self.db)
            .render_transformed(&self.db, ffi_js_mod_path)
            .clone())
    }

    /// Removes all modules from a profile and invalidates its cached build.
    ///
    /// Keeps the profile's configuration, dependency watchers, and existing
    /// artifacts. Returns an error if the profile is unknown.
    #[napi]
    pub fn reset(&mut self, profile: String) -> Result<()> {
        let state = self.profile(&profile)?;
        let program = state.program;
        let revision = state.revision.clone();
        program.set_files(&mut self.db).to(Vec::new());
        revision.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Removes a module and its snippets from a profile, invalidating the cached
    /// build if the module was present.
    ///
    /// An absent module is a no-op. Returns an error if the profile is unknown.
    #[napi]
    pub fn remove_module(&mut self, profile: String, module_id: String) -> Result<()> {
        let state = self.profile(&profile)?;
        let program = state.program;
        let revision = state.revision.clone();
        let files = program
            .files(&self.db)
            .iter()
            .copied()
            .filter(|file| file.module_id(&self.db) != &module_id)
            .collect::<Vec<_>>();
        if files.len() != program.files(&self.db).len() {
            program.set_files(&mut self.db).to(files);
            revision.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Builds a profile, reusing its last successful result while inputs are unchanged.
    ///
    /// Successful builds receive a new artifact directory; previous artifacts are
    /// retained. Compiler diagnostics are returned without a binary path on failure.
    /// Unknown profiles, dependency inspection failures, and I/O failures return errors.
    #[napi]
    pub fn build(&mut self, profile: String) -> Result<BuildResult> {
        self.profile(&profile)?;
        if self.watch_graph_dirty.swap(false, Ordering::SeqCst) {
            if let Some(callback) = self.watch_callback.clone() {
                if let Err(error) = self.configure_watch(callback) {
                    self.watch_graph_dirty.store(true, Ordering::SeqCst);
                    return Err(error);
                }
            }
        }
        let state = self
            .profiles
            .get_mut(&profile)
            .ok_or_else(|| Error::from_reason(format!("unknown profile: {profile}")))?;
        state.build(&self.db)
    }

    /// Watches configured crates and their local dependencies for all profiles.
    ///
    /// Changes invalidate affected builds and notify `callback` with the profile
    /// name or a watcher error. Generated output is excluded. Replaces existing
    /// watchers on success; dependency inspection or watcher setup failures return errors.
    #[napi(ts_args_type = "callback: (change: DependencyChange) => void")]
    pub fn watch(&mut self, callback: ChangeCallback) -> Result<()> {
        self.configure_watch(Arc::new(callback))
    }

    /// Stops dependency watching and releases the callback.
    ///
    /// Programs and build artifacts remain available. Calling this repeatedly is safe.
    #[napi]
    pub fn close(&mut self) {
        self.watchers.clear();
        self.watch_callback = None;
    }
}

impl Server {
    fn configure_watch(&mut self, callback: Arc<ChangeCallback>) -> Result<()> {
        let mut watchers = Vec::new();
        for (name, profile) in &self.profiles {
            let mut directories = BTreeMap::new();
            for path in profile.crates.values() {
                let output = Command::new("cargo")
                    .args(["metadata", "--format-version=1", "--manifest-path"])
                    .arg(Path::new(path).join("Cargo.toml"))
                    .output()
                    .map_err(|error| {
                        Error::from_reason(format!("cannot inspect crate dependencies: {error}"))
                    })?;
                if !output.status.success() {
                    return Err(Error::from_reason(
                        String::from_utf8_lossy(&output.stderr).into_owned(),
                    ));
                }
                let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
                    .map_err(|error| Error::from_reason(error.to_string()))?;
                for package in metadata["packages"].as_array().into_iter().flatten() {
                    if package["source"].is_null() {
                        if let Some(manifest) = package["manifest_path"].as_str() {
                            directories.insert(
                                PathBuf::from(manifest).parent().unwrap().to_owned(),
                                RecursiveMode::Recursive,
                            );
                        }
                    }
                }
                if let Some(root) = metadata["workspace_root"].as_str() {
                    directories
                        .entry(PathBuf::from(root))
                        .or_insert(RecursiveMode::NonRecursive);
                }
            }
            if directories.is_empty() {
                continue;
            }
            let name = name.clone();
            let revision = profile.revision.clone();
            let watch_graph_dirty = self.watch_graph_dirty.clone();
            let callback = callback.clone();
            let mut watcher =
                notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                    let error = match event {
                        Ok(event) => {
                            if matches!(event.kind, notify::EventKind::Access(_))
                                || !event.paths.iter().any(|path| {
                                    !path.components().any(|component| {
                                        matches!(
                                            component.as_os_str().to_str(),
                                            Some("target" | ".rustier" | ".git")
                                        )
                                    })
                                })
                            {
                                return;
                            }
                            if event.paths.iter().any(|path| {
                                matches!(
                                    path.file_name().and_then(|name| name.to_str()),
                                    Some("Cargo.toml" | "Cargo.lock")
                                )
                            }) {
                                watch_graph_dirty.store(true, Ordering::SeqCst);
                            }
                            None
                        }
                        Err(error) => Some(error.to_string()),
                    };
                    revision.fetch_add(1, Ordering::SeqCst);
                    let _ = callback.call(
                        DependencyChange {
                            profile: name.clone(),
                            error,
                        },
                        ThreadsafeFunctionCallMode::NonBlocking,
                    );
                })
                .map_err(|error| Error::from_reason(error.to_string()))?;
            for (directory, mode) in directories {
                watcher
                    .watch(&directory, mode)
                    .map_err(|error| Error::from_reason(error.to_string()))?;
            }
            watchers.push(watcher);
        }
        self.watchers = watchers;
        self.watch_callback = Some(callback);
        Ok(())
    }

    fn profile(&self, name: &str) -> Result<&Profile> {
        self.profiles
            .get(name)
            .ok_or_else(|| Error::from_reason(format!("unknown profile: {name}")))
    }
}

impl Profile {
    fn build(&mut self, db: &RootDatabase) -> Result<BuildResult> {
        let revision = self.revision.load(Ordering::SeqCst);
        if let Some((built_revision, result)) = &self.built {
            if *built_revision == revision {
                return Ok(result.clone());
            }
        }
        let mut result = BuildResult {
            diagnostics: self
                .program
                .diagnostics(db)
                .iter()
                .map(|diagnostic| diagnostic.render(Theme::UnicodeNoColor))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|error| Error::from_reason(error.to_string()))?,
            bin_ffi_path: None,
        };
        if !result.diagnostics.is_empty() {
            return Ok(result);
        }
        fs::create_dir_all(&self.directory).map_err(|error| {
            Error::from_reason(format!("cannot create bridge directory: {error}"))
        })?;
        let source = self.program.runtime(db).render_rs_ffi(db, &self.backend);
        let manifest = self.program.render_cargo_toml(db, &self.crates);
        write_if_changed(&self.directory.join("lib.rs"), source.as_bytes())
            .and_then(|_| write_if_changed(&self.directory.join("Cargo.toml"), manifest.as_bytes()))
            .map_err(|error| Error::from_reason(format!("cannot write bridge crate: {error}")))?;

        let target = match &self.backend {
            TransportBackend::Wasm(wasm) => Some(if wasm.mem64 {
                "wasm64-unknown-unknown"
            } else {
                "wasm32-unknown-unknown"
            }),
            TransportBackend::Node(_) => None,
        };
        let mut command = Command::new("cargo");
        command.args(["build", "--quiet"]);
        if let Some(target) = target {
            command.args(["--target", target]);
        } else if cfg!(target_os = "macos") {
            command.args(["--config", "build.rustflags=[\"-C\", \"link-arg=-undefined\", \"-C\", \"link-arg=dynamic_lookup\"]"]);
        }
        command
            .arg("--manifest-path")
            .arg(self.directory.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(self.directory.join("target"));
        if self.release {
            command.arg("--release");
        }
        let build = command
            .output()
            .map_err(|error| Error::from_reason(format!("cannot run Cargo: {error}")))?;
        let diagnostics = String::from_utf8_lossy(&build.stderr).trim().to_owned();
        if !diagnostics.is_empty() {
            result.diagnostics.push(diagnostics);
        }
        if !build.status.success() {
            if result.diagnostics.is_empty() {
                result
                    .diagnostics
                    .push(format!("Cargo exited with {}", build.status));
            }
            return Ok(result);
        }

        let profile = if self.release { "release" } else { "debug" };
        let mut source = self.directory.join("target");
        if let Some(target) = target {
            source.push(target);
        }
        source.push(profile);
        let (library, artifact) = match &self.backend {
            TransportBackend::Wasm(_) => ("rustier_macros.wasm".to_owned(), "bridge.wasm"),
            TransportBackend::Node(_) => (
                format!(
                    "{}rustier_macros{}",
                    std::env::consts::DLL_PREFIX,
                    std::env::consts::DLL_SUFFIX
                ),
                "bridge.node",
            ),
        };
        source.push(library);
        // A fresh directory also defeats Node's native addon cache. Retain old
        // generations: in-flight requests may still refer to their binaries.
        let artifact_directory = self.directory.join("artifacts").join(format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&artifact_directory)
            .and_then(|_| fs::copy(source, artifact_directory.join(artifact)))
            .map_err(|error| {
                Error::from_reason(format!("cannot store compiled binary: {error}"))
            })?;
        result.bin_ffi_path = Some(
            artifact_directory
                .join(artifact)
                .to_string_lossy()
                .into_owned(),
        );
        self.built = Some((revision, result.clone()));
        Ok(result)
    }
}

/// Preserve mtimes so Cargo can reuse the bridge when only JavaScript changed.
fn write_if_changed(path: &Path, contents: &[u8]) -> io::Result<()> {
    match fs::read(path) {
        Ok(previous) if previous == contents => Ok(()),
        Ok(_) => fs::write(path, contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::write(path, contents),
        Err(error) => Err(error),
    }
}
