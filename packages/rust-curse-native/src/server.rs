#![allow(clippy::result_large_err)]

use std::path::PathBuf;

use napi_derive::napi;

use crate::{
    compiler::codegen::shim::{Platform, WasmEnv},
    server::{
        project::ProjectError,
        toolchain::{ToolInstallationOpts, Toolchain, ToolchainOps},
        workspace::ServerWorkspace,
    },
};

mod diag;
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
    #[napi]
    pub fn create(cfg: Config) -> ServerCreateResult {
        (|| -> Result<Self, ServerDiagnostic> {
            let manifest_path = PathBuf::from(cfg.manifest_path);
            let ws = ServerWorkspace::load(manifest_path).map_err(ServerDiagnostic::from)?;

            let tools_dir = ws.project().out_dir().join("tools");
            let toolchain = Toolchain::install(ToolchainOps {
                napi: ToolInstallationOpts::Install(&tools_dir.join("napi")),
                wasm_bindgen: ToolInstallationOpts::Install(&tools_dir.join("wasm_bindgen")),
                cargo: ToolInstallationOpts::Discover,
            })
            .map_err(ServerDiagnostic::from)?;

            Ok(Self { ws, toolchain })
        })()
        .into()
    }

    #[napi]
    pub fn change(&mut self, change: ServerChange) -> ServerChangeResult {
        (|| -> Result<(), ServerDiagnostic> {
            self.ws
                .change(change.into())
                .map_err(ServerDiagnostic::from)?;
            let db = self.ws.db();
            self.ws
                .project()
                .ts()
                .sync(db)
                .exec(db)
                .map_err(ServerDiagnostic::from)?;
            Ok(())
        })()
        .into()
    }

    #[napi]
    pub fn build(&mut self, platform: ServerPlatform, release: bool) -> ServerBuildResult {
        let result = (|| -> Result<Vec<String>, ProjectError> {
            let project = self.ws.project();
            project.ts().sync(self.ws.db()).exec(self.ws.db())?;

            let cursed_crate = project.cursed_crate(platform.into());
            cursed_crate.sync(self.ws.db()).exec()?;

            let artifact = cursed_crate
                .build(self.ws.db(), release)
                .exec(self.ws.db_mut(), &self.toolchain)?;
            Ok(vec![artifact.bin_path.as_str().to_owned()])
        })();

        result
            .map(|bin_paths| ServerBuildOutput { bin_paths })
            .map_err(ServerDiagnostic::from)
            .into()
    }

    #[napi]
    pub fn check(&self) -> Vec<ServerDiagnostic> {
        let db = self.ws.db();
        self.ws
            .project()
            .reachable_shim(db)
            .check(db)
            .into_iter()
            .cloned()
            .map(ServerDiagnostic::from)
            .collect()
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

#[napi]
pub enum ServerDiagnosticSeverity {
    Error,
    Warning,
    Advice,
}

#[napi(object)]
pub struct ServerDiagnostic {
    pub code: Option<String>,
    pub message: String,
    pub severity: ServerDiagnosticSeverity,
    pub file: Option<String>,
    pub loc: Option<ServerDiagnosticLocation>,
    pub url: Option<String>,
    /// Complete rendered report, including labels, help, and related
    /// diagnostics.
    pub rendered: String,
}

#[napi(object)]
pub struct ServerDiagnosticLocation {
    pub file: Option<String>,
    /// One-based line number.
    pub line: u32,
    /// Zero-based UTF-16 column, matching JavaScript source positions.
    pub column: u32,
}

macro_rules! server_result {
    ($name:ident, $value:ty) => {
        #[napi_derive::napi(object_from_js = false)]
        #[allow(clippy::large_enum_variant)]
        pub enum $name {
            Ok { value: $value },
            Err { error: ServerDiagnostic },
        }

        impl From<Result<$value, ServerDiagnostic>> for $name {
            fn from(result: Result<$value, ServerDiagnostic>) -> Self {
                match result {
                    Ok(value) => Self::Ok { value },
                    Err(error) => Self::Err { error },
                }
            }
        }
    };
}

server_result!(ServerCreateResult, Server);
server_result!(ServerChangeResult, ());
server_result!(ServerBuildResult, ServerBuildOutput);
