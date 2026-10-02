use std::{collections::HashMap, path::PathBuf};

use salsa::Setter;

use crate::{
    compiler::{codegen::js::JsCodegenPaths, db::Db, path::FilePath},
    server::toolchain::{
        CargoBuildCommand, Toolchain, ToolchainError, WasmBindgenCommand, WasmBindgenTarget,
    },
};

use super::{
    CursedCrate, Project,
    common::{ArtifactError, reserve_unique_artifact_dir},
};

/// How the generated WebAssembly module is initialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum WasmLoading {
    Fetch,
    ModuleNode,
    Module,
}

impl WasmLoading {
    pub fn js(self, path: &str, wasm_path: &str) -> String {
        match self {
            Self::Fetch => {
                format!("import init from {path:?};\nawait init();\nexport * from {path:?};\n")
            }
            Self::ModuleNode => format!("export * from {path:?};\n"),
            Self::Module => format!(
                "import wasmModule from {wasm_path:?};\nimport init from {path:?};\n\
                 await init({{ module_or_path: wasmModule }});\nexport * from {path:?};\n"
            ),
        }
    }
}

#[salsa::interned(debug)]
pub struct CursedCrateWasm<'db> {
    pub(super) project: Project<'db>,
}

impl<'a> CursedCrateWasm<'a> {
    pub const fn name(self) -> &'static str {
        "wasm"
    }

    fn build_artifacts(self, db: &'a dyn Db) -> Option<&'a BuildArtifactWasm> {
        BuildArtifactsWasm::artifact(
            db,
            CursedCrate::Wasm(self)
                .manifest_path(db)
                .to_string_lossy()
                .as_ref(),
        )
    }

    pub fn js_ffi_file_path(self, db: &'a dyn Db) -> Option<&'a FilePath> {
        Some(&self.build_artifacts(db)?.ffi_js_file)
    }

    pub fn bin_ffi_file_path(self, db: &'a dyn Db) -> Option<&'a FilePath> {
        Some(&self.build_artifacts(db)?.bin_file)
    }

    pub fn js(self, db: &'a dyn Db, path: &str, loading: WasmLoading) -> Option<String> {
        const FFI_MODULE_ID: &str = "$rust/unstable!/wasm";

        if path == FFI_MODULE_ID {
            return Some(loading.js(
                self.js_ffi_file_path(db)?.as_str(),
                self.bin_ffi_file_path(db)?.as_str(),
            ));
        }

        Some(
            self.project(db)
                .js_module(db, path)?
                .render(
                    db,
                    JsCodegenPaths {
                        ffi_js_mod_id: FFI_MODULE_ID.to_owned(),
                    },
                )
                .clone(),
        )
    }

    /// WebAssembly build of the cursed crate.
    ///
    /// Doesn't do anything until executed.
    pub fn build(
        self,
        db: &'a dyn Db,
        release: bool,
        loading: WasmLoading,
        dep_features: &[String],
    ) -> CursedCrateWasmBuild {
        let crate_ = CursedCrate::Wasm(self);
        CursedCrateWasmBuild {
            out_dir: crate_.root_dir(db).clone(),
            release,
            target: match loading {
                WasmLoading::Fetch | WasmLoading::Module => WasmBindgenTarget::Web,
                WasmLoading::ModuleNode => WasmBindgenTarget::NodeJsModule,
            },
            features: crate_.dependency_features(db, dep_features),
        }
    }
}

/// An owned WASM build plan, executed after releasing the database borrow.
#[derive(Debug, Clone)]
pub struct CursedCrateWasmBuild {
    out_dir: PathBuf,
    release: bool,
    target: WasmBindgenTarget,
    features: Vec<String>,
}

impl CursedCrateWasmBuild {
    pub fn exec(
        &self,
        db: &mut dyn Db,
        toolchain: &Toolchain,
    ) -> Result<FilePath, CursedCrateWasmBuildError> {
        let manifest_path = self.out_dir.join("Cargo.toml");
        let key = manifest_path.to_string_lossy();
        BuildArtifactsWasm::remove(db, &key);

        let artifact_dir = reserve_unique_artifact_dir(&self.out_dir)?;
        let output = CargoBuildCommand {
            manifest_path: &manifest_path,
            target_dir: &self.out_dir.join("target"),
            target: Some("wasm32-unknown-unknown"),
            release: self.release,
            features: &self.features,
        }
        .exec(toolchain)?;
        let binary = output
            .artifacts
            .into_iter()
            .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("wasm"))
            .ok_or(CursedCrateWasmBuildError::MissingLibraryArtifact)?;
        let output = WasmBindgenCommand {
            wasm_path: &binary,
            output_dir: &artifact_dir,
            target: self.target,
        }
        .exec(toolchain)?;

        let bin_file =
            FilePath::try_from(output.wasm_path.as_path()).map_err(ArtifactError::from)?;
        BuildArtifactsWasm::set(
            db,
            key.into_owned(),
            BuildArtifactWasm {
                ffi_js_file: FilePath::try_from(output.js_path.as_path())
                    .map_err(ArtifactError::from)?,
                bin_file: bin_file.clone(),
            },
        );
        Ok(bin_file)
    }
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum CursedCrateWasmBuildError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Toolchain(#[from] ToolchainError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Artifact(#[from] ArtifactError),
    #[error("Cargo did not produce a library artifact for the requested package")]
    #[diagnostic(code(rust_curse::missing_library_artifact))]
    MissingLibraryArtifact,
}

#[salsa::input(singleton)]
struct BuildArtifactsWasm {
    /// The latest wasm-bindgen output for each manifest.
    #[returns(ref)]
    artifacts: HashMap<String, BuildArtifactWasm>,
}

impl BuildArtifactsWasm {
    fn remove(db: &mut dyn Db, key: &str) {
        if let Some(input) = Self::try_get(db)
            && input.artifacts(db).contains_key(key)
        {
            let mut artifacts = input.artifacts(db).clone();
            artifacts.remove(key);
            input.set_artifacts(db).to(artifacts);
        }
    }

    fn set(db: &mut dyn Db, key: String, artifact: BuildArtifactWasm) {
        let input = Self::try_get(db).unwrap_or_else(|| Self::new(db, HashMap::default()));
        let mut artifacts = input.artifacts(db).clone();
        artifacts.insert(key, artifact);
        input.set_artifacts(db).to(artifacts);
    }

    fn artifact<'db>(db: &'db dyn Db, key: &str) -> Option<&'db BuildArtifactWasm> {
        Self::try_get(db).and_then(|input| input.artifacts(db).get(key))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, salsa::Update)]
pub struct BuildArtifactWasm {
    pub ffi_js_file: FilePath,
    pub bin_file: FilePath,
}
