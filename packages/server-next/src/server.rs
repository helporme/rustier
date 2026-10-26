use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use compiler::{
    codegen::{TransportBackend, TransportBackendNode, TransportBackendWasm, WasmImport},
    db::RootDatabase,
    diag::Theme,
    ir::js::{JsFile, JsProgram},
};
use napi::{Error, Result};
use napi_derive::napi;
use salsa::Setter;

#[napi(object)]
pub struct ServerOptions {
    pub root_dir: String,
    pub release: Option<bool>,
    pub backend: Option<String>,
    pub mem64: Option<bool>,
    pub wasm_import: Option<String>,
    pub crates: Option<HashMap<String, String>>,
}

#[napi(object)]
pub struct BuildResult {
    pub diagnostics: Vec<String>,
    pub bin_ffi_path: Option<String>,
}

#[napi]
pub struct Server {
    db: RootDatabase,
    program: JsProgram,
    directory: PathBuf,
    release: bool,
    backend: TransportBackend,
    crates: BTreeMap<String, String>,
    next_artifact: u64,
}

#[napi]
impl Server {
    #[napi(constructor)]
    pub fn new(options: ServerOptions) -> Result<Self> {
        let root = fs::canonicalize(&options.root_dir)
            .map_err(|error| Error::from_reason(format!("invalid project root: {error}")))?;
        let db = RootDatabase::default();
        let program = JsProgram::new(&db, Vec::new());
        let import = match options.wasm_import.as_deref().unwrap_or("fetch") {
            "fetch" => WasmImport::Fetch,
            "node-js" => WasmImport::NodeJs,
            "module" => WasmImport::Module,
            mode => {
                return Err(Error::from_reason(format!(
                    "unsupported Wasm import mode: {mode}"
                )));
            }
        };
        let backend = match options.backend.as_deref().unwrap_or("wasm") {
            "wasm" => TransportBackend::Wasm(TransportBackendWasm {
                mem64: options.mem64.unwrap_or(false),
                import,
            }),
            "node" => TransportBackend::Node(TransportBackendNode),
            backend => {
                return Err(Error::from_reason(format!(
                    "unsupported backend: {backend}"
                )));
            }
        };
        let directory = root.join(".rustier/bridge");
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
            .collect::<Result<BTreeMap<_, _>>>()?;

        Ok(Self {
            db,
            program,
            directory,
            release: options.release.unwrap_or(false),
            backend,
            crates,
            next_artifact: 0,
        })
    }

    /// Complete JavaScript runtime for the collected source files.
    #[napi]
    pub fn render_js_ffi(&self, bin_file_path: String) -> String {
        self.program
            .runtime(&self.db)
            .render_js_ffi(&self.db, &self.backend, &bin_file_path)
    }

    #[napi]
    pub fn transform(
        &mut self,
        module_id: String,
        contents: String,
        ffi_js_mod_path: String,
    ) -> Result<String> {
        let file = JsFile::new(&self.db, module_id.clone(), contents);
        let mut files = self.program.files(&self.db).clone();
        match files
            .iter_mut()
            .find(|entry| entry.module_id(&self.db) == &module_id)
        {
            Some(entry) => *entry = file,
            None => files.push(file),
        }
        self.program.set_files(&mut self.db).to(files);

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

    #[napi]
    pub fn build(&mut self) -> Result<BuildResult> {
        let artifacts = self.directory.join("artifacts");
        fs::create_dir_all(&artifacts).map_err(|error| {
            Error::from_reason(format!("cannot create artifacts directory: {error}"))
        })?;
        for entry in fs::read_dir(&artifacts)
            .map_err(|error| Error::from_reason(format!("cannot list artifacts: {error}")))?
        {
            let entry = entry.map_err(|error| Error::from_reason(error.to_string()))?;
            let Some(number) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u64>().ok())
            else {
                continue;
            };
            if entry
                .file_type()
                .map_err(|error| Error::from_reason(error.to_string()))?
                .is_dir()
            {
                let next = number
                    .checked_add(1)
                    .ok_or_else(|| Error::from_reason("artifact counter exhausted"))?;
                self.next_artifact = self.next_artifact.max(next);
                fs::remove_dir_all(entry.path()).map_err(|error| {
                    Error::from_reason(format!("cannot remove previous artifact: {error}"))
                })?;
            }
        }
        let number = self.next_artifact;
        self.next_artifact = number
            .checked_add(1)
            .ok_or_else(|| Error::from_reason("artifact counter exhausted"))?;
        let artifact_directory = artifacts.join(number.to_string());
        fs::create_dir_all(&artifact_directory).map_err(|error| {
            Error::from_reason(format!("cannot create artifact directory: {error}"))
        })?;
        let mut result = BuildResult {
            diagnostics: Vec::new(),
            bin_ffi_path: None,
        };
        result.diagnostics = self
            .program
            .diagnostics(&self.db)
            .iter()
            .map(|diagnostic| diagnostic.render(Theme::UnicodeNoColor))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| Error::from_reason(error.to_string()))?;
        if !result.diagnostics.is_empty() {
            return Ok(result);
        }

        let source = self
            .program
            .runtime(&self.db)
            .render_rs_ffi(&self.db, &self.backend);
        let manifest = self.program.render_cargo_toml(&self.db, &self.crates);
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
        if let Some(target) = target {
            command.args(["build", "--quiet", "--target", target]);
        } else {
            command.args(["build", "--quiet"]);
            if cfg!(target_os = "macos") {
                command.args(["--config", "build.rustflags=[\"-C\", \"link-arg=-undefined\", \"-C\", \"link-arg=dynamic_lookup\"]"]);
            }
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
        let binary = artifact_directory.join(artifact);
        fs::copy(source, &binary).map_err(|error| {
            Error::from_reason(format!("cannot store compiled binary: {error}"))
        })?;

        result.bin_ffi_path = Some(binary.to_string_lossy().into_owned());

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
