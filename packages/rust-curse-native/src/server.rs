#![allow(clippy::result_large_err)]

use std::{collections::HashMap, path::PathBuf};

use napi::threadsafe_function::ThreadsafeFunction;
use napi_derive::napi;

use crate::{
    compiler::codegen::shim::{Platform, WasmEnv},
    server::{
        log::ServerLogs,
        project::ProjectError,
        toolchain::{ToolInstallationOpts, Toolchain, ToolchainOps},
        workspace::ServerWorkspace,
    },
};

mod diag;
mod log;
mod project;
mod target;
mod toolchain;
mod workspace;
mod write_file;

pub use target::ServerNodeTarget;

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
    logs: ServerLogs,
}

#[napi]
impl Server {
    #[napi]
    pub fn create(cfg: Config) -> ServerCreateResult {
        (|| -> Result<Self, ServerDiagnostic> {
            let logs = ServerLogs::new();
            let _dispatch = tracing::dispatcher::set_default(logs.dispatch());
            let manifest_path = PathBuf::from(cfg.manifest_path);

            let ws = ServerWorkspace::load(manifest_path).map_err(ServerDiagnostic::from)?;

            let tools_dir = ws.project().out_dir().join("tools");
            let toolchain = Toolchain::install(ToolchainOps {
                wasm_bindgen: ToolInstallationOpts::Install(&tools_dir.join("wasm_bindgen")),
                cargo: ToolInstallationOpts::Discover,
            })
            .map_err(ServerDiagnostic::from)?;

            Ok(Self {
                ws,
                toolchain,
                logs,
            })
        })()
        .into()
    }

    #[napi]
    pub fn on_log(
        &self,
        callback: Option<ThreadsafeFunction<ServerLog, (), ServerLog, napi::Status, false, true>>,
    ) {
        self.logs.on_log(callback);
    }

    #[napi]
    pub fn change(&mut self, change: ServerChange) -> ServerChangeResult {
        let _dispatch = tracing::dispatcher::set_default(self.logs.dispatch());
        tracing::debug!(path = %change.file_path, "Updating Rust source");
        (|| -> Result<(), ServerDiagnostic> {
            self.ws
                .change(change.into())
                .map_err(ServerDiagnostic::from)?;

            self.ws
                .with_db(|db| self.ws.project().ts().sync(db).exec(db))
                .map_err(ServerDiagnostic::from)?;
            Ok(())
        })()
        .into()
    }

    #[napi]
    pub fn build(&mut self, platform: ServerPlatform, release: bool) -> ServerBuildResult {
        let _dispatch = tracing::dispatcher::set_default(self.logs.dispatch());

        let node_target = match &platform {
            ServerPlatform::Node { target } => *target,
            ServerPlatform::Wasm { .. } => None,
        };
        let platform = Platform::from(platform);
        let platform_label = match platform {
            Platform::Node => "node",
            Platform::Wasm {
                target: WasmEnv::Node,
            } => "webasm-node",
            Platform::Wasm {
                target: WasmEnv::Web,
            } => "webasm",
        };

        let profile = if release { "release" } else { "debug" };

        let _span = tracing::info_span!("Rust build", platform = platform_label, profile).entered();

        let result = (|| -> Result<Vec<String>, ProjectError> {
            let project = self.ws.project();
            {
                let _span = tracing::info_span!(parent: None, "TypeScript declaration generation")
                    .entered();
                self.ws.with_db(|db| project.ts().sync(db).exec(db))?;
            }

            let cursed_crate = project.cursed_crate(platform);
            {
                let _span = tracing::debug_span!("Rust shim generation").entered();
                self.ws.with_db(|db| cursed_crate.sync(db).exec())?;
            }

            let bin_path = {
                let _span = tracing::info_span!("Rust artifact compilation").entered();
                let build = self.ws.with_db(|db| match platform {
                    Platform::Node => cursed_crate.build_napi(db, release, node_target),
                    Platform::Wasm { .. } => cursed_crate.build_wasm(db, release),
                });
                self.ws.with_db_mut(|db| build.exec(db, &self.toolchain))?;
                self.ws.with_db(|db| {
                    self.ws
                        .project()
                        .cursed_crate(platform)
                        .bin_file(db, node_target)
                        .expect("successful build has a binary")
                        .as_str()
                        .to_owned()
                })
            };
            Ok(vec![bin_path])
        })();

        if let Err(error) = &result {
            tracing::error!(%error, "Rust build failed");
        }

        result
            .map(|bin_paths| ServerBuildOutput { bin_paths })
            .map_err(ServerDiagnostic::from)
            .into()
    }

    #[napi]
    pub fn check(&self) -> Vec<ServerDiagnostic> {
        let _dispatch = tracing::dispatcher::set_default(self.logs.dispatch());
        let _span = tracing::info_span!("Rust API check").entered();
        self.ws.with_db(|db| {
            let shim = self.ws.project().reachable_shim(db);
            let diagnostics = shim.check(db);

            diagnostics
                .into_iter()
                .cloned()
                .map(ServerDiagnostic::from)
                .collect()
        })
    }

    #[napi]
    pub fn load(&self, platform: ServerPlatform, path: String) -> Option<String> {
        let _dispatch = tracing::dispatcher::set_default(self.logs.dispatch());
        let _span = tracing::debug_span!("Rust facade rendering", %path).entered();

        self.ws.with_db(|db| {
            self.ws
                .project()
                .cursed_crate(platform.into())
                .js(db, &path)
                .cloned()
        })
    }
}

/// The platform to build for.
#[napi]
pub enum ServerPlatform {
    Wasm { target: ServerWasmTarget },
    Node { target: Option<ServerNodeTarget> },
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
            ServerPlatform::Node { .. } => Platform::Node,
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

#[napi]
pub enum ServerLogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[napi(object)]
pub struct ServerLog {
    pub level: ServerLogLevel,
    pub target: String,
    pub message: String,
    /// Span fields followed by event fields; the innermost value wins.
    pub fields: HashMap<String, String>,
}
