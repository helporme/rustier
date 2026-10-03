use std::{collections::HashMap, path::PathBuf};

use salsa::Setter;

use crate::{
    compiler::{codegen::js::JsCodegenPaths, db::Db, path::FilePath},
    server::{
        ServerNodeTarget,
        toolchain::{CargoBuildCommand, Toolchain, ToolchainError},
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

    fn build_artifacts(self, db: &'a dyn Db) -> Option<&'a BuildArtifactNode> {
        BuildArtifactsNode::artifact(
            db,
            BridgeCrate::Node(self)
                .manifest_path(db)
                .to_string_lossy()
                .as_ref(),
        )
    }

    pub fn js_ffi_file_path(self, db: &'a dyn Db) -> Option<&'a FilePath> {
        Some(&self.build_artifacts(db)?.ffi_js_file)
    }

    pub fn bin_ffi_file_path(
        self,
        db: &'a dyn Db,
        target: Option<ServerNodeTarget>,
    ) -> Option<&'a FilePath> {
        self.build_artifacts(db)?.bin_files.get(&target)
    }

    pub fn js(self, db: &'a dyn Db, path: &str) -> Option<String> {
        const FFI_MODULE_ID: &str = "$rust/unstable!/node";
        if path == FFI_MODULE_ID {
            return Some(format!(
                "export * from {:?};\n",
                self.js_ffi_file_path(db)?.as_str()
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
        release: bool,
        node_target: Option<ServerNodeTarget>,
        dep_features: &[String],
    ) -> BridgeCrateNodeBuild {
        let crate_ = BridgeCrate::Node(self);
        BridgeCrateNodeBuild {
            out_dir: crate_.root_dir(db).clone(),
            release,
            node_target,
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
#[derive(Debug, Clone)]
pub struct BridgeCrateNodeBuild {
    out_dir: PathBuf,
    release: bool,
    node_target: Option<ServerNodeTarget>,
    napi_ffi: String,
    features: Vec<String>,
}

impl BridgeCrateNodeBuild {
    pub fn exec(
        &self,
        db: &mut dyn Db,
        toolchain: &Toolchain,
    ) -> Result<FilePath, BridgeCrateNodeBuildError> {
        let manifest_path = self.out_dir.join("Cargo.toml");
        let key = manifest_path.to_string_lossy();
        let mut bin_files = BuildArtifactsNode::artifact(db, &key)
            .map(|artifact| artifact.bin_files.clone())
            .unwrap_or_default();
        bin_files.remove(&self.node_target);
        BuildArtifactsNode::remove(db, &key);

        let target = self
            .node_target
            .or_else(ServerNodeTarget::host)
            .ok_or(BridgeCrateNodeBuildError::UnsupportedNodeHost)?;
        let artifact_dir = reserve_unique_artifact_dir(&self.out_dir)?;
        let output = CargoBuildCommand {
            manifest_path: &manifest_path,
            target_dir: &self.out_dir.join("target"),
            target: Some(target.triple()),
            release: self.release,
            features: &self.features,
        }
        .exec(toolchain)?;
        let binary = output
            .artifacts
            .into_iter()
            .find(|path| {
                matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("dylib" | "dll" | "so")
                )
            })
            .ok_or(BridgeCrateNodeBuildError::MissingLibraryArtifact)?;

        for file in bin_files.values_mut() {
            let destination = artifact_dir.join(file.as_path().file_name().unwrap());
            *file = copy_artifact(file.as_path(), &destination)?;
        }

        let name = target.native_binary_name();
        let bin_file = copy_artifact(&binary, &artifact_dir.join(name))?;
        let js_path = artifact_dir.join("index.mjs");
        write_file(&js_path, &self.napi_ffi)?;

        bin_files.insert(self.node_target, bin_file.clone());
        BuildArtifactsNode::set(
            db,
            key.into_owned(),
            BuildArtifactNode {
                ffi_js_file: FilePath::try_from(js_path.as_path()).map_err(ArtifactError::from)?,
                bin_files,
            },
        );
        Ok(bin_file)
    }
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum BridgeCrateNodeBuildError {
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

#[salsa::input(singleton)]
struct BuildArtifactsNode {
    /// The latest loader and native binaries for each manifest.
    #[returns(ref)]
    artifacts: HashMap<String, BuildArtifactNode>,
}

impl BuildArtifactsNode {
    fn remove(db: &mut dyn Db, key: &str) {
        if let Some(input) = Self::try_get(db)
            && input.artifacts(db).contains_key(key)
        {
            let mut artifacts = input.artifacts(db).clone();
            artifacts.remove(key);
            input.set_artifacts(db).to(artifacts);
        }
    }

    fn set(db: &mut dyn Db, key: String, artifact: BuildArtifactNode) {
        let input = Self::try_get(db).unwrap_or_else(|| Self::new(db, HashMap::default()));
        let mut artifacts = input.artifacts(db).clone();
        artifacts.insert(key, artifact);
        input.set_artifacts(db).to(artifacts);
    }

    fn artifact<'db>(db: &'db dyn Db, key: &str) -> Option<&'db BuildArtifactNode> {
        Self::try_get(db).and_then(|input| input.artifacts(db).get(key))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, salsa::Update)]
pub struct BuildArtifactNode {
    pub ffi_js_file: FilePath,
    pub bin_files: HashMap<Option<ServerNodeTarget>, FilePath>,
}
