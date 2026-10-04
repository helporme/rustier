use std::path::PathBuf;

use crate::{
    compiler::{codegen::js::JsCodegenPaths, db::Db, path::FilePath},
    server::{
        ServerNodeTarget,
        effects::Effect,
        toolchain::{CargoBuild, Toolchain, ToolchainError},
        write_file::{WriteFileError, write_file},
    },
};

use super::{
    BridgeCrate, Project,
    common::{ArtifactError, copy_artifact, reserve_unique_artifact_dir},
};

#[salsa::interned(debug)]
pub struct BridgeCrateNode<'db> {
    pub(super) project: Project<'db>,
}

impl<'a> BridgeCrateNode<'a> {
    pub const fn name(self) -> &'static str {
        "node"
    }

    pub fn js(
        self,
        db: &'a dyn Db,
        path: &str,
        artifact: Option<&NodeBuildArtifact>,
    ) -> Option<String> {
        const FFI_MODULE_ID: &str = "$rust/unstable!/node";
        if path == FFI_MODULE_ID {
            return Some(format!(
                "export * from {:?};\n",
                artifact?.js_path.as_str()
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

    /// Native build of the bridge crate.
    ///
    /// Doesn't do anything until executed.
    pub fn build(
        self,
        db: &'a dyn Db,
        workspace_version: u128,
        release: bool,
        target: Option<ServerNodeTarget>,
        dep_features: &[String],
    ) -> NodeBuild {
        let crate_ = BridgeCrate::Node(self);
        NodeBuild {
            out_dir: crate_.root_dir(db).clone(),
            workspace_version,
            release,
            target,
            features: crate_.dependency_features(db, dep_features),
            napi_ffi: self
                .project(db)
                .reachable_shim(db)
                .facade(db)
                .ffi(db)
                .render_napi_ffi(db),
        }
    }
}

/// An owned native build plan, executed after releasing the database borrow.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeBuild {
    out_dir: PathBuf,
    workspace_version: u128,
    release: bool,
    target: Option<ServerNodeTarget>,
    napi_ffi: String,
    features: Vec<String>,
}

impl Effect for NodeBuild {
    type Args<'a> = &'a Toolchain;
    type Res = NodeBuildArtifact;
    type Err = NodeBuildError;

    fn _exec<'a>(
        &self,
        db: &mut dyn Db,
        toolchain: Self::Args<'a>,
    ) -> Result<NodeBuildArtifact, NodeBuildError> {
        let manifest_path = self.out_dir.join("Cargo.toml");
        let target = self
            .target
            .or_else(ServerNodeTarget::host)
            .ok_or(NodeBuildError::UnsupportedNodeHost)?;

        let output = CargoBuild {
            workspace_version: self.workspace_version,
            manifest_path,
            target_dir: self.out_dir.join("target"),
            target: Some(target.triple()),
            release: self.release,
            features: self.features.clone(),
        }
        .run(db, toolchain)?;
        let binary = output
            .artifacts
            .into_iter()
            .find(|path| {
                matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("dylib" | "dll" | "so")
                )
            })
            .ok_or(NodeBuildError::MissingLibraryArtifact)?;

        let artifact_dir = reserve_unique_artifact_dir(&self.out_dir)?;
        let name = target.native_binary_name();
        let bin_path = copy_artifact(&binary, &artifact_dir.join(name))?;
        let js_path = artifact_dir.join("index.mjs");
        write_file(&js_path, &self.napi_ffi)?;

        Ok(NodeBuildArtifact {
            bin_path,
            js_path: FilePath::try_from(js_path.as_path()).map_err(ArtifactError::from)?,
        })
    }

    fn _synced(&self, _db: &dyn Db, artifact: &NodeBuildArtifact) -> Result<bool, NodeBuildError> {
        Ok(artifact.bin_path.as_path().is_file() && artifact.js_path.as_path().is_file())
    }
}

#[derive(Debug, Clone)]
pub struct NodeBuildArtifact {
    pub bin_path: FilePath,
    pub js_path: FilePath,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum NodeBuildError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Toolchain(#[from] ToolchainError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    WriteFile(#[from] WriteFileError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Artifact(#[from] ArtifactError),
    #[error("Cargo did not produce a library artifact for the requested package")]
    #[diagnostic(code(rust_curse::missing_library_artifact))]
    MissingLibraryArtifact,
    #[error("the host target is not supported for native Node builds")]
    UnsupportedNodeHost,
}
