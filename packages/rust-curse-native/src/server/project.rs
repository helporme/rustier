use std::{
    collections::HashMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use salsa::Setter;

use crate::{
    compiler::{
        codegen::{
            js::JsCodegenPaths,
            shim::{
                Platform, WasmEnv,
                manifest::{ManifestOpts, render_manifest},
            },
        },
        db::Db,
        ir::{
            rust::{Crate, Rust, usages::UsageGraph},
            shim::Shim,
        },
        path::{FilePath, FilePathError},
    },
    server::{
        ServerNodeTarget,
        project::ts::ProjectTs,
        toolchain::{
            CargoBuildCommand, Toolchain, ToolchainError, WasmBindgenCommand, WasmBindgenTarget,
        },
        write_file::{WriteFileError, write_file},
    },
};

pub mod ts;

#[derive(Copy, Clone, Debug)]
pub struct Project<'a> {
    /// Manifest path of the project crate
    manifest_path: &'a Path,
    /// Path to the root file of the project (lib.rs)
    root_file: &'a Path,
    /// Cargo package name, which can differ from the Rust library target name.
    package_name: &'a str,
}

impl<'a> Project<'a> {
    pub fn new(manifest_path: &'a Path, root_file: &'a Path, package_name: &'a str) -> Self {
        assert!(manifest_path.is_file() && manifest_path.file_name().unwrap() == "Cargo.toml");
        assert!(root_file.is_file() && root_file.file_name().unwrap() == "lib.rs");

        Self {
            manifest_path,
            root_file,
            package_name,
        }
    }

    /// Cursed crate of the project.
    pub fn cursed_crate(self, platform: Platform) -> CursedCrate<'a> {
        CursedCrate {
            project: self,
            platform,
        }
    }

    /// Source crate of the project.
    pub fn src_crate(self, db: &dyn Db) -> Crate<'_> {
        *Rust::get(db)
            .crates(db)
            .iter()
            .find(|c| c.is_local(db) && c.root(db).file_path(db).as_path() == self.root_file)
            .expect("Project must have a local crate")
    }

    /// A shim reachable from the project's local crate.
    pub fn reachable_shim(self, db: &dyn Db) -> Shim<'_> {
        Rust::get(db).shim_reachable_from(db, UsageGraph::new(db, [self.src_crate(db).root(db)]))
    }

    /// Project root directory.
    pub fn root_dir(self) -> &'a Path {
        self.manifest_path
            .parent()
            .expect("Cargo.toml must be in a directory")
    }

    /// Output directory.
    pub fn out_dir(self) -> PathBuf {
        self.root_dir().join(".rust-curse")
    }

    /// Typescript configuration of the project.
    pub fn ts(self) -> ProjectTs<'a> {
        ProjectTs::new(self)
    }
}

/// A glue crate that contains rendered bindings code from source rust crate
/// to the JavaScript.
#[derive(Copy, Clone, Debug)]
pub struct CursedCrate<'a> {
    project: Project<'a>,
    platform: Platform,
}

impl<'a> CursedCrate<'a> {
    /// Root directory of the cursed crate.
    pub fn root_dir(self, db: &dyn Db) -> PathBuf {
        let name = self.name(db);
        self.project.out_dir().join("crates").join(name)
    }

    /// Cursed crate name.
    pub fn name(self, db: &dyn Db) -> String {
        let suffix = match self.platform {
            Platform::Wasm { .. } => "wasm",
            Platform::Node => "node",
        };

        format!("{}_{suffix}", self.project.src_crate(db).name(db))
    }

    /// Path to the manifest file of the cursed crate.
    pub fn manifest_path(self, db: &dyn Db) -> PathBuf {
        self.root_dir(db).join("Cargo.toml")
    }

    pub fn ffi_js_file(self, db: &dyn Db) -> Option<&FilePath> {
        BuildArtifacts::artifact(db, self.manifest_path(db).to_string_lossy().as_ref())
            .map(|artifact| &artifact.ffi_js_file)
    }

    pub fn bin_file(self, db: &dyn Db, target: Option<ServerNodeTarget>) -> Option<&FilePath> {
        BuildArtifacts::artifact(db, self.manifest_path(db).to_string_lossy().as_ref())
            .and_then(|artifact| artifact.bin_files.get(&target))
    }

    pub fn ffi_js(self, db: &dyn Db) -> Option<String> {
        // FIXME: move to the compiler

        let ffi_js_file = self.ffi_js_file(db)?;
        // Debug formatting quotes and escapes the filesystem path.
        let specifier = format!("{:?}", ffi_js_file.as_str());
        let mut module = format!("export * from {specifier};\n");
        if matches!(
            self.platform,
            Platform::Wasm {
                target: WasmEnv::Web
            }
        ) {
            module.push_str(&format!(
                "import init from {specifier};\nawait init();\nexport {{ default }} from {specifier};\n"
            ));
        }
        Some(module)
    }

    pub fn js(self, db: &dyn Db, path: &str) -> Option<String> {
        let ffi_module_id = match self.platform {
            Platform::Node => "$rust/unstable!/node",
            Platform::Wasm { .. } => "$rust/unstable!/wasm",
        };

        if path == ffi_module_id {
            return self.ffi_js(db);
        }

        self.ffi_js_file(db)?;
        self.project
            .reachable_shim(db)
            .crates(db)
            .flat_map(|c| c.all_modules(db))
            .find(|m| {
                m.decals(db).virtual_path(db) == path || m.src(db).file_path(db).as_str() == path
            })
            .map(|m| {
                m.facade(db)
                    .render(
                        db,
                        JsCodegenPaths {
                            ffi_js_mod_id: ffi_module_id.to_owned(),
                        },
                    )
                    .clone()
            })
    }

    /// Synchronization of the cursed crate to the output directory.
    ///
    /// Doesn't do anything until executed.
    pub fn sync(self, db: &dyn Db) -> CursedCrateSync {
        let name = self.name(db);

        let manifest = render_manifest(ManifestOpts {
            name: &name,
            source_package_name: self.project.package_name,
            source_crate_name: self.project.src_crate(db).name(db),
            source_package_path: self.project.root_dir(),
            platform: self.platform,
        });

        let lib = self
            .project
            .reachable_shim(db)
            .render(db, self.platform)
            .clone();
        let build = self
            .platform
            .build_script_body()
            .map(|body| format!("fn main() {{ {body} }}\n"));

        CursedCrateSync {
            out_dir: self.root_dir(db),
            manifest,
            lib,
            build,
        }
    }

    /// Native build of the cursed crate.
    ///
    /// Doesn't do anything until executed.
    pub fn build_napi(
        self,
        db: &dyn Db,
        release: bool,
        node_target: Option<ServerNodeTarget>,
    ) -> CursedCrateBuild {
        CursedCrateBuild {
            out_dir: self.root_dir(db),
            platform: self.platform,
            release,
            node_target,
            napi_ffi: Some(
                self.project
                    .reachable_shim(db)
                    .facade(db)
                    .ffi(db)
                    .render_napi_ffi(db),
            ),
        }
    }

    /// WebAssembly build of the cursed crate.
    ///
    /// Doesn't do anything until executed.
    pub fn build_wasm(self, db: &dyn Db, release: bool) -> CursedCrateBuild {
        CursedCrateBuild {
            out_dir: self.root_dir(db),
            platform: self.platform,
            release,
            node_target: None,
            napi_ffi: None,
        }
    }
}

/// Synchronization of the cursed crate to the output directory.
#[derive(Debug, Clone)]
pub struct CursedCrateSync {
    out_dir: PathBuf,
    manifest: String,
    lib: String,
    build: Option<String>,
}

impl CursedCrateSync {
    pub fn exec(&self) -> Result<(), ProjectError> {
        let CursedCrateSync {
            out_dir,
            manifest,
            lib,
            build,
        } = self;

        write_file(out_dir.join("Cargo.toml"), manifest)?;
        write_file(out_dir.join("src/lib.rs"), lib)?;
        if let Some(build) = build {
            write_file(out_dir.join("build.rs"), build)?;
        }
        Ok(())
    }
}

/// The build of a cursed crate.
#[derive(Debug, Clone)]
pub struct CursedCrateBuild {
    out_dir: PathBuf,
    platform: Platform,
    release: bool,
    node_target: Option<ServerNodeTarget>,
    napi_ffi: Option<String>,
}

impl CursedCrateBuild {
    pub fn exec(&self, db: &mut dyn Db, toolchain: &Toolchain) -> Result<(), ProjectError> {
        let manifest_path = self
            .out_dir
            .join("Cargo.toml")
            .to_string_lossy()
            .into_owned();

        let mut bin_files = BuildArtifacts::artifact(db, &manifest_path)
            .map(|artifact| artifact.bin_files.clone())
            .unwrap_or_default();
        bin_files.remove(&self.node_target);
        BuildArtifacts::remove(db, &manifest_path);

        let artifact_dir = self.reserve_unique_artifact_dir()?;

        let output = CargoBuildCommand {
            manifest_path: &self.out_dir.join("Cargo.toml"),
            target_dir: &self.out_dir.join("target"),
            target: Some(match self.platform {
                Platform::Node => self
                    .node_target
                    .map_or(env!("RUST_CURSE_TARGET"), ServerNodeTarget::triple),
                Platform::Wasm { .. } => "wasm32-unknown-unknown",
            }),
            release: self.release,
        }
        .exec(toolchain)?;

        let binary = output
            .artifacts
            .into_iter()
            .find(|path| {
                let extension = path.extension().and_then(|ext| ext.to_str());
                match self.platform {
                    Platform::Node => matches!(extension, Some("dylib" | "dll" | "so")),
                    Platform::Wasm { .. } => extension == Some("wasm"),
                }
            })
            .ok_or(ProjectError::MissingLibraryArtifact)?;

        for file in bin_files.values_mut() {
            let destination = artifact_dir.join(file.as_path().file_name().unwrap());
            *file = copy_artifact(file.as_path(), &destination)?;
        }

        let (ffi_js_file, bin_file): (FilePath, FilePath) = match self.platform {
            Platform::Node => {
                let name = self
                    .node_target
                    .or_else(ServerNodeTarget::host)
                    .map(ServerNodeTarget::native_binary_name)
                    .ok_or(ProjectError::UnsupportedNodeHost)?;
                let bin_file = copy_artifact(&binary, &artifact_dir.join(name))?;
                let js_path = artifact_dir.join("index.mjs");
                write_file(&js_path, self.napi_ffi.as_ref().unwrap())?;
                (js_path.as_path().try_into()?, bin_file)
            }
            Platform::Wasm { target } => {
                let output = WasmBindgenCommand {
                    wasm_path: &binary,
                    output_dir: &artifact_dir,
                    target: match target {
                        WasmEnv::Web => WasmBindgenTarget::Web,
                        WasmEnv::Node => WasmBindgenTarget::NodeJsModule,
                    },
                }
                .exec(toolchain)?;
                (
                    output.js_path.as_path().try_into()?,
                    output.wasm_path.as_path().try_into()?,
                )
            }
        };

        bin_files.insert(self.node_target, bin_file);
        BuildArtifacts::set(
            db,
            manifest_path,
            BuildArtifact {
                ffi_js_file,
                bin_files,
            },
        );
        Ok(())
    }

    /// Reserve a unique artifact directory on disk to which the build output
    /// will be written. The node.js cached the .node addon by file path,
    /// and it cannot be reloaded as far as i can tell, so we need to came up
    /// with a new path each time to support watch mode.
    fn reserve_unique_artifact_dir(&self) -> Result<PathBuf, ProjectError> {
        let root = self.out_dir.join("artifacts");
        let io_error = |operation| {
            let path = root.clone();
            move |source| ProjectError::ArtifactIo {
                operation,
                path,
                source,
            }
        };
        fs::create_dir_all(&root).map_err(io_error("create artifact directory"))?;
        let mut version = 0_u64;
        for entry in fs::read_dir(&root).map_err(io_error("read artifact directory"))? {
            let entry = entry.map_err(io_error("read artifact entry"))?;
            if let Some(existing) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u64>().ok())
            {
                version = version.max(
                    existing
                        .checked_add(1)
                        .ok_or(ProjectError::ArtifactVersionOverflow)?,
                );
            }
        }

        loop {
            let dir = root.join(version.to_string());
            match fs::create_dir(&dir) {
                Ok(()) => return Ok(dir),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    version = version
                        .checked_add(1)
                        .ok_or(ProjectError::ArtifactVersionOverflow)?;
                }
                Err(source) => {
                    return Err(ProjectError::ArtifactIo {
                        operation: "reserve artifact directory",
                        path: dir,
                        source,
                    });
                }
            }
        }
    }
}

fn copy_artifact(source: &Path, destination: &Path) -> Result<FilePath, ProjectError> {
    fs::copy(source, destination).map_err(|source| ProjectError::ArtifactIo {
        operation: "copy artifact to",
        path: destination.to_owned(),
        source,
    })?;
    Ok(destination.try_into()?)
}

#[salsa::input(singleton)]
struct BuildArtifacts {
    /// A shared JS loader and target-specific binaries for each manifest.
    #[returns(ref)]
    artifacts: HashMap<String, BuildArtifact>,
}

impl BuildArtifacts {
    pub fn remove(db: &mut dyn Db, key: &str) {
        if let Some(input) = Self::try_get(db)
            && input.artifacts(db).contains_key(key)
        {
            let mut artifacts = input.artifacts(db).clone();
            artifacts.remove(key);
            input.set_artifacts(db).to(artifacts);
        }
    }

    pub fn set(db: &mut dyn Db, key: String, artifact: BuildArtifact) {
        let input = Self::try_get(db).unwrap_or_else(|| Self::new(db, HashMap::default()));

        let mut artifacts = input.artifacts(db).clone();
        artifacts.insert(key, artifact);
        input.set_artifacts(db).to(artifacts);
    }

    pub fn artifact<'db>(db: &'db dyn Db, key: &str) -> Option<&'db BuildArtifact> {
        Self::try_get(db).and_then(|input| input.artifacts(db).get(key))
    }
}

/// Absolute paths to the shared loader and the binaries built so far.
#[derive(Debug, Clone, Eq, PartialEq, salsa::Update)]
pub struct BuildArtifact {
    pub ffi_js_file: FilePath,
    pub bin_files: HashMap<Option<ServerNodeTarget>, FilePath>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum ProjectError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Toolchain(#[from] ToolchainError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    WriteFile(#[from] WriteFileError),
    #[error("invalid build artifact path: {0}")]
    #[diagnostic(code(rust_curse::invalid_artifact_path))]
    ArtifactPath(#[from] FilePathError),
    #[error("failed to {operation} {}", path.display())]
    #[diagnostic(code(rust_curse::artifact_io))]
    ArtifactIo {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("artifact version overflow")]
    #[diagnostic(code(rust_curse::artifact_version_overflow))]
    ArtifactVersionOverflow,
    #[error("Cargo did not produce a library artifact for the requested package")]
    #[diagnostic(code(rust_curse::missing_library_artifact))]
    MissingLibraryArtifact,
    #[error("the host target is not supported for native Node builds")]
    UnsupportedNodeHost,
    #[error("local module {} is outside project root {}", path.display(), root.display())]
    #[diagnostic(code(rust_curse::module_outside_project))]
    ModuleOutsideRoot { path: PathBuf, root: PathBuf },
}
