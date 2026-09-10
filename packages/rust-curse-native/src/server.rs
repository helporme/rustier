use std::path::{Path, PathBuf};

use napi_derive::napi;

use crate::{
    compiler::codegen::shim::{Platform, WasmEnv},
    server::{
        toolchain::{ToolInstallationOpts, Toolchain, ToolchainOps},
        workspace::ServerWorkspace,
    },
};

mod project;
mod toolchain;
mod workspace;
mod write_file;

#[napi(object)]
pub struct Config {
    /// Root directory of the workspace.
    pub root_dir: String,
    /// Manifest path of the "main" crate relative to the [`root_dir`].
    pub manifest_path: String,
}

/// A change to the server's workspace.
#[napi(object)]
pub struct ServerChange {
    pub file_path: String,
    pub contents: Option<String>,
}

/// The rust-curse server.
#[napi]
pub struct Server {
    ws: ServerWorkspace,
    toolchain: Toolchain,
}

#[napi]
impl Server {
    #[napi(constructor)]
    pub fn new(cfg: Config) -> napi::Result<Self> {
        let manifest_path: PathBuf = PathBuf::from(cfg.manifest_path);

        let ws = ServerWorkspace::load(manifest_path)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;

        let tools_dir = ws.project().out_dir().join("tools");
        let toolchain = Toolchain::install(ToolchainOps {
            napi: ToolInstallationOpts::Install(&tools_dir.join("napi")),
            wasm_bindgen: ToolInstallationOpts::Install(&tools_dir.join("wasm_bindgen")),
            cargo: ToolInstallationOpts::Discover,
        })
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

        Ok(Self { ws, toolchain })
    }

    #[napi]
    pub fn change(&mut self, change: ServerChange) -> napi::Result<()> {
        self.ws
            .change(change.into())
            .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;

        let db = self.ws.db();
        self.ws
            .project()
            .ts()
            .sync(db)
            .exec(db)
            .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;

        Ok(())
    }

    #[napi]
    pub fn build(
        &mut self,
        platform: ServerPlatform,
        release: bool,
    ) -> napi::Result<ServerBuildOutput> {
        let project = self.ws.project();

        project
            .ts()
            .sync(self.ws.db())
            .exec(self.ws.db())
            .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;

        let cursed_crate = project.cursed_crate(platform.into());

        cursed_crate
            .sync(self.ws.db())
            .exec()
            .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;

        let artifact = cursed_crate
            .build(self.ws.db(), release)
            .exec(self.ws.db_mut(), &self.toolchain)
            .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;

        Ok(ServerBuildOutput {
            bin_paths: vec![artifact.bin_path.as_str().to_owned()],
        })
    }

    #[napi]
    pub fn load(&self, platform: ServerPlatform, path: String) -> Option<String> {
        let db = self.ws.db();

        self.ws
            .project()
            .cursed_crate(platform.into())
            .js(db, &path)
            .cloned()
    }
}

/// The platform to build for.
#[napi]
pub enum ServerPlatform {
    Wasm { target: ServerWasmTarget },
    Node,
}

impl From<ServerPlatform> for Platform {
    fn from(value: ServerPlatform) -> Self {
        match value {
            ServerPlatform::Wasm { target } => Platform::Wasm {
                target: match target {
                    ServerWasmTarget::Web => WasmEnv::Web,
                    ServerWasmTarget::Node => WasmEnv::Node,
                },
            },
            ServerPlatform::Node => Platform::Node,
        }
    }
}

/// The target platform for the WASM build.
#[napi]
pub enum ServerWasmTarget {
    Web,
    Node,
}

/// The output of a server build.
#[napi(object)]
pub struct ServerBuildOutput {
    /// The paths to the built binaries that should be located next to the
    /// project's built .js file in dist folder.
    pub bin_paths: Vec<String>,
}
