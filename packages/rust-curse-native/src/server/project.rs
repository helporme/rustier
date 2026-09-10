use std::{
    collections::HashMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
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
        path::FilePath,
    },
    server::{
        project::ts::ProjectTs,
        toolchain::{
            CargoBuildCommand, NapiBuildCommand, Toolchain, WasmBindgenCommand, WasmBindgenTarget,
        },
        write_file::write_file,
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
        Rust::get(db).shim_reachable_from(UsageGraph::new(db, [self.src_crate(db).root(db)]))
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

    /// Latest built artifact for the cursed crate.
    ///
    /// See the [`build`][`Self::build`] for obtaining a build and executing it.
    pub fn artifact(self, db: &dyn Db) -> Option<&BuildArtifact> {
        BuildArtifacts::artifact(db, self.manifest_path(db).to_string_lossy().as_ref())
    }

    pub fn js<'db>(self, db: &'db dyn Db, path: &String) -> Option<&'db String> {
        if let Some(artifact) = self.artifact(db) {
            self.project
                .reachable_shim(db)
                .crates(db)
                .flat_map(|c| c.all_modules(db))
                .find(|m| {
                    m.decals(db).virtual_path(db) == path
                        || m.src(db).file_path(db).as_str() == path
                })
                .map(|m| {
                    m.facade(db).render(
                        db,
                        JsCodegenPaths {
                            ffi_js_file: artifact.js_path.clone(),
                        },
                    )
                })
        } else {
            None
        }
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

        let lib = self.project.reachable_shim(db).render(db, self.platform);
        let build = self
            .platform
            .build_script_body()
            .map(|body| format!("fn main() {{ {body} }}\n"));

        let package = matches!(self.platform, Platform::Node).then_some(format!(
            // It's technically possible to just write `{}` into the package.json, but we need to
            // have some decency
            r#"
            {{
                "name": "{name}",
                "version": "0.0.0",
                "private": true
            }}
            "#
        ));

        CursedCrateSync {
            out_dir: self.root_dir(db),
            manifest,
            lib,
            build,
            package,
        }
    }

    /// Build of the cursed crate for the given platform and release mode.
    ///
    /// Doesn't do anything until executed.
    pub fn build(self, db: &dyn Db, release: bool) -> CursedCrateBuild {
        CursedCrateBuild {
            out_dir: self.root_dir(db),
            platform: self.platform,
            release,
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
    /// package.json contents for node projects that use napi
    package: Option<String>,
}

impl CursedCrateSync {
    pub fn exec(&self) -> anyhow::Result<()> {
        let CursedCrateSync {
            out_dir,
            manifest,
            lib,
            build,
            package,
        } = self;

        write_file(out_dir.join("Cargo.toml"), manifest)?;
        write_file(out_dir.join("src/lib.rs"), lib)?;
        if let Some(build) = build {
            write_file(out_dir.join("build.rs"), build)?;
        }
        if let Some(package) = package {
            write_file(out_dir.join("package.json"), package)?;
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
}

impl CursedCrateBuild {
    pub fn exec<'db>(
        &self,
        db: &'db mut dyn Db,
        toolchain: &Toolchain,
    ) -> anyhow::Result<&'db BuildArtifact> {
        let artifact = match self.platform {
            Platform::Node => self.build_node(toolchain),
            Platform::Wasm { target } => self.build_wasm(toolchain, target),
        }?;

        let manifest_path = self
            .out_dir
            .join("Cargo.toml")
            .to_string_lossy()
            .into_owned();

        Ok(BuildArtifacts::set(db, manifest_path, artifact))
    }

    fn build_node(&self, toolchain: &Toolchain) -> anyhow::Result<BuildArtifact> {
        let artifact_dir = self.reserve_unique_artifact_dir()?;
        let output = NapiBuildCommand {
            manifest_path: &self.out_dir.join("Cargo.toml"),
            output_dir: &artifact_dir,
            release: self.release,
        }
        .exec(toolchain)?;

        Ok(BuildArtifact {
            js_path: output.js_path.as_path().try_into()?,
            bin_path: output.node_path.as_path().try_into()?,
        })
    }

    fn build_wasm(&self, toolchain: &Toolchain, target: WasmEnv) -> anyhow::Result<BuildArtifact> {
        let artifact_dir = self.reserve_unique_artifact_dir()?;

        let output = CargoBuildCommand {
            manifest_path: &self.out_dir.join("Cargo.toml"),
            target_dir: &artifact_dir,
            target: Some("wasm32-unknown-unknown"),
            release: self.release,
        }
        .exec(toolchain)?;

        let wasm_path = output
            .artifacts
            .into_iter()
            .find(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "wasm")
            })
            .ok_or_else(|| anyhow::anyhow!("expected one .wasm artifact, found none"))?;

        let output = WasmBindgenCommand {
            wasm_path: &wasm_path,
            output_dir: &artifact_dir,
            target: match target {
                WasmEnv::Web => WasmBindgenTarget::Web,
                WasmEnv::Node => WasmBindgenTarget::NodeJs,
            },
        }
        .exec(toolchain)?;

        Ok(BuildArtifact {
            js_path: output.js_path.as_path().try_into()?,
            bin_path: output.wasm_path.as_path().try_into()?,
        })
    }

    /// Reserve a unique artifact directory on disk to which the build output
    /// will be written. The node.js cached the .node addon by file path,
    /// and it cannot be reloaded as far as i can tell, so we need to came up
    /// with a new path each time to support watch mode.
    fn reserve_unique_artifact_dir(&self) -> anyhow::Result<PathBuf> {
        let root = self.out_dir.join("artifacts");
        fs::create_dir_all(&root)?;
        let mut version = 0_u64;
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            if let Some(existing) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u64>().ok())
            {
                version = version.max(
                    existing
                        .checked_add(1)
                        .context("artifact version overflow")?,
                );
            }
        }

        fs::remove_dir_all(&root)?;
        fs::create_dir_all(&root)?;

        loop {
            let dir = root.join(version.to_string());
            match fs::create_dir(&dir) {
                Ok(()) => return Ok(dir),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    version = version
                        .checked_add(1)
                        .context("artifact version overflow")?;
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("reserve artifacts in {}", dir.display()));
                }
            }
        }
    }
}

#[salsa::input(singleton)]
struct BuildArtifacts {
    /// A map of manifest paths to their build artifacts.
    #[returns(ref)]
    artifacts: HashMap<String, BuildArtifact>,
}

impl BuildArtifacts {
    pub fn set(db: &mut dyn Db, manifest_path: String, artifact: BuildArtifact) -> &BuildArtifact {
        let input = Self::try_get(db).unwrap_or_else(|| Self::new(db, HashMap::default()));

        let mut artifacts = input.artifacts(db).clone();
        artifacts.insert(manifest_path.clone(), artifact);
        input.set_artifacts(db).to(artifacts);

        Self::artifact(db, &manifest_path).unwrap()
    }

    pub fn artifact<'db>(db: &'db dyn Db, manifest_path: &str) -> Option<&'db BuildArtifact> {
        Self::try_get(db).and_then(|input| input.artifacts(db).get(manifest_path))
    }
}

/// Absolute paths to a build's platform-specific outputs.
#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub struct BuildArtifact {
    pub js_path: FilePath,
    pub bin_path: FilePath,
}
