use std::path::PathBuf;

use crate::{
    compiler::{
        codegen::{
            abi::js::{AbiJsTarget, WasmLoading},
            js::JsCodegenPaths,
        },
        db::Db,
        ir::abi::transport::{WasmMemory, WasmTransport},
        path::FilePath,
    },
    server::{
        effects::Effect,
        toolchain::{CargoBuild, Toolchain, ToolchainError, WasmBindgen, WasmBindgenTarget},
    },
};

use super::{
    BridgeCrate, Project,
    common::{ArtifactError, reserve_unique_artifact_dir},
};

impl WasmLoading {
    pub(in crate::server) fn bindgen_target(self) -> WasmBindgenTarget {
        match self {
            Self::Fetch | Self::Module => WasmBindgenTarget::Web,
            Self::ModuleNode => WasmBindgenTarget::NodeJsModule,
        }
    }
}

#[salsa::interned(debug)]
pub struct BridgeCrateWasm<'db> {
    pub(super) project: Project<'db>,
    pub(super) transport: WasmTransport,
}

impl<'a> BridgeCrateWasm<'a> {
    pub fn name(self, db: &'a dyn Db) -> &'static str {
        match self.transport(db).memory {
            WasmMemory::x32 => "wasm",
            WasmMemory::x64 => "wasm64",
        }
    }

    pub fn js(
        self,
        db: &'a dyn Db,
        path: &str,
        loading: WasmLoading,
        artifact: Option<&WasmBuildArtifact>,
    ) -> Option<String> {
        const ABI_MODULE_ID: &str = "$rust/unstable!/wasm";

        if path == ABI_MODULE_ID {
            let artifact = artifact?;

            return Some(self.project(db).reachable_facade(db).runtime(db).render_js(
                db,
                AbiJsTarget::Wasm {
                    loading,
                    path: artifact.js_path.as_str(),
                    wasm_path: artifact.bin_path.as_str(),
                },
            ));
        }

        Some(self.project(db).js_module(db, path)?.render_js(
            db,
            JsCodegenPaths {
                abi_js_mod_id: ABI_MODULE_ID.to_owned(),
            },
        ))
    }

    /// WebAssembly build of the bridge crate.
    ///
    /// Doesn't do anything until executed.
    pub fn build(
        self,
        db: &'a dyn Db,
        workspace_version: u128,
        release: bool,
        loading: WasmLoading,
        dep_features: &[String],
    ) -> WasmBuild {
        let crate_ = BridgeCrate::Wasm(self);
        WasmBuild {
            out_dir: crate_.root_dir(db).clone(),
            workspace_version,
            release,
            target: loading.bindgen_target(),
            memory: self.transport(db).memory,
            features: crate_.dependency_features(db, dep_features),
        }
    }
}

/// An owned WASM build plan, executed after releasing the database borrow.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WasmBuild {
    out_dir: PathBuf,
    workspace_version: u128,
    release: bool,
    target: WasmBindgenTarget,
    memory: WasmMemory,
    features: Vec<String>,
}

impl Effect for WasmBuild {
    type Args<'a> = &'a Toolchain;
    type Res = WasmBuildArtifact;
    type Err = WasmBuildError;

    fn _exec<'a>(
        &self,
        db: &mut dyn Db,
        toolchain: Self::Args<'a>,
    ) -> Result<WasmBuildArtifact, WasmBuildError> {
        let manifest_path = self.out_dir.join("Cargo.toml");
        let artifact_dir = reserve_unique_artifact_dir(&self.out_dir)?;
        let output = CargoBuild {
            workspace_version: self.workspace_version,
            manifest_path,
            target_dir: self.out_dir.join("target"),
            target: Some(match self.memory {
                WasmMemory::x32 => "wasm32-unknown-unknown",
                WasmMemory::x64 => "wasm64-unknown-unknown",
            }),
            release: self.release,
            features: self.features.clone(),
        }
        .run(db, toolchain)?;

        let binary = output
            .artifacts
            .into_iter()
            .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("wasm"))
            .ok_or(WasmBuildError::MissingLibraryArtifact)?;

        let output = WasmBindgen {
            workspace_version: self.workspace_version,
            wasm_path: binary,
            output_dir: artifact_dir,
            name_suffix: match self.target {
                WasmBindgenTarget::Web => "web",
                WasmBindgenTarget::NodeJsModule => "nodejs_module",
            },
            target: self.target,
        }
        .run(db, toolchain)?;

        Ok(WasmBuildArtifact {
            bin_path: FilePath::try_from(output.wasm_path.as_path())
                .map_err(ArtifactError::from)?,
            js_path: FilePath::try_from(output.js_path.as_path()).map_err(ArtifactError::from)?,
        })
    }

    fn _synced(&self, _db: &dyn Db, artifact: &WasmBuildArtifact) -> Result<bool, WasmBuildError> {
        Ok(artifact.bin_path.as_path().is_file() && artifact.js_path.as_path().is_file())
    }
}

#[derive(Debug, Clone)]
pub struct WasmBuildArtifact {
    pub bin_path: FilePath,
    pub js_path: FilePath,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum WasmBuildError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Toolchain(#[from] ToolchainError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Artifact(#[from] ArtifactError),
    #[error("Cargo did not produce a library artifact for the requested package")]
    #[diagnostic(code(rustier::missing_library_artifact))]
    MissingLibraryArtifact,
}
