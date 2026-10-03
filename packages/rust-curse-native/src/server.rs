#![allow(clippy::result_large_err)]

use std::{collections::HashMap, path::PathBuf};

use napi::threadsafe_function::ThreadsafeFunction;
use napi_derive::napi;

use crate::{
    compiler::codegen::shim::Platform,
    server::{
        log::ServerLogs,
        project::{CursedCrateNode, CursedCrateWasm, WasmLoading},
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
    /// Whether to generate TypeScript declarations for the Rust API.
    pub ts: bool,
}

/// A change to the server's workspace.
#[napi(object)]
pub struct ServerChange {
    pub file_path: String,
    pub contents: Option<String>,
}

/// Options for compiling a Rust runtime.
#[napi(object)]
pub struct BuildOpts {
    pub platform: ServerPlatform,
    pub release: bool,
    pub features: Option<Vec<String>>,
}

/// The rust-curse server.
#[napi]
pub struct Server {
    config: Config,
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
            let root_dir = PathBuf::from(&cfg.root_dir);
            let manifest_path = root_dir.join(&cfg.manifest_path);
            let ws = ServerWorkspace::load(manifest_path).map_err(ServerDiagnostic::from)?;

            let tools_dir = ws.with_db(|db| ws.project(db).out_dir(db).join("tools"));
            let toolchain = Toolchain::install(ToolchainOps {
                wasm_bindgen: ToolInstallationOpts::Install(&tools_dir.join("wasm_bindgen")),
                cargo: ToolInstallationOpts::Discover,
            })
            .map_err(ServerDiagnostic::from)?;

            Ok(Self {
                config: cfg,
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
        (|| -> Result<bool, ServerDiagnostic> {
            let changed = self
                .ws
                .change(change.into())
                .map_err(ServerDiagnostic::from)?;

            if changed && self.config.ts {
                self.ws
                    .with_db(|db| self.ws.project(db).ts().sync(db).exec(db))
                    .map_err(ServerDiagnostic::from)?;
            }
            Ok(changed)
        })()
        .into()
    }

    #[napi]
    pub fn build(&mut self, opts: BuildOpts) -> ServerBuildResult {
        let _dispatch = tracing::dispatcher::set_default(self.logs.dispatch());
        let BuildOpts {
            platform,
            release,
            features,
        } = opts;
        let features = features.unwrap_or_default();

        let compiler_platform = match &platform {
            ServerPlatform::Node { .. } => Platform::Node,
            ServerPlatform::Wasm { .. } => Platform::Wasm,
        };
        let platform_label = match &platform {
            ServerPlatform::Node { .. } => "node",
            ServerPlatform::Wasm {
                target: ServerWasmLoading::ModuleNode,
            } => "webasm-node",
            ServerPlatform::Wasm {
                target: ServerWasmLoading::Fetch,
            } => "webasm",
            ServerPlatform::Wasm {
                target: ServerWasmLoading::Module,
            } => "webasm-module",
        };

        let profile = if release { "release" } else { "debug" };

        let _span = tracing::info_span!("Rust build", platform = platform_label, profile).entered();

        let result = (|| -> Result<ServerBuildOutput, ServerDiagnostic> {
            if self.config.ts {
                let _span = tracing::info_span!(parent: None, "TypeScript declaration generation")
                    .entered();
                self.ws
                    .with_db(|db| self.ws.project(db).ts().sync(db).exec(db))
                    .map_err(ServerDiagnostic::from)?;
            }

            {
                let _span = tracing::debug_span!("Rust shim generation").entered();
                self.ws
                    .with_db(|db| {
                        self.ws
                            .project(db)
                            .cursed_crate(db, compiler_platform)
                            .sync(db)
                            .exec()
                    })
                    .map_err(ServerDiagnostic::from)?;
            }

            let bin_path = {
                let _span = tracing::info_span!("Rust artifact compilation").entered();
                match platform {
                    ServerPlatform::Node { target } => {
                        let build = self.ws.with_db(|db| {
                            CursedCrateNode::new(db, self.ws.project(db))
                                .build(db, release, target, &features)
                        });
                        self.ws
                            .with_db_mut(|db| build.exec(db, &self.toolchain))
                            .map_err(ServerDiagnostic::from)?
                    }
                    ServerPlatform::Wasm { target } => {
                        let loading = WasmLoading::from(target);
                        let build = self.ws.with_db(|db| {
                            CursedCrateWasm::new(db, self.ws.project(db))
                                .build(db, release, loading, &features)
                        });
                        self.ws
                            .with_db_mut(|db| build.exec(db, &self.toolchain))
                            .map_err(ServerDiagnostic::from)?
                    }
                }
            };
            Ok(ServerBuildOutput {
                bin_paths: vec![bin_path.as_str().to_owned()],
            })
        })();

        if let Err(error) = &result {
            tracing::error!(error = %error.message, "Rust build failed");
        }

        result.into()
    }

    #[napi]
    pub fn check(&self) -> Vec<ServerDiagnostic> {
        let _dispatch = tracing::dispatcher::set_default(self.logs.dispatch());
        let _span = tracing::info_span!("Rust API check").entered();
        self.ws.with_db(|db| {
            let shim = self.ws.project(db).reachable_shim(db);
            let diagnostics = shim.check(db);

            diagnostics
                .into_iter()
                .cloned()
                .map(ServerDiagnostic::from)
                .collect()
        })
    }

    /// Current binary paths for the platform. Empty if no build is available.
    #[napi]
    pub fn bin_paths(&self, platform: ServerPlatform) -> Vec<String> {
        self.ws.with_db(|db| {
            let project = self.ws.project(db);
            let binary = match platform {
                ServerPlatform::Node { target } => {
                    CursedCrateNode::new(db, project).bin_ffi_file_path(db, target)
                }
                ServerPlatform::Wasm { target } => {
                    CursedCrateWasm::new(db, project).bin_ffi_file_path(db, target.into())
                }
            };
            binary
                .into_iter()
                .map(|path| path.as_str().to_owned())
                .collect()
        })
    }

    #[napi]
    pub fn load(&self, platform: ServerPlatform, path: String) -> Option<String> {
        let _dispatch = tracing::dispatcher::set_default(self.logs.dispatch());
        let _span = tracing::debug_span!("Rust facade rendering", %path).entered();

        self.ws.with_db(|db| {
            let project = self.ws.project(db);
            match platform {
                ServerPlatform::Node { .. } => CursedCrateNode::new(db, project).js(db, &path),
                ServerPlatform::Wasm { target } => {
                    CursedCrateWasm::new(db, project).js(db, &path, target.into())
                }
            }
        })
    }
}

/// The platform to build for.
#[napi]
pub enum ServerPlatform {
    Wasm { target: ServerWasmLoading },
    Node { target: Option<ServerNodeTarget> },
}

impl From<ServerPlatform> for Platform {
    fn from(value: ServerPlatform) -> Self {
        match value {
            ServerPlatform::Wasm { .. } => Platform::Wasm,
            ServerPlatform::Node { .. } => Platform::Node,
        }
    }
}

/// The loading method for the WASM build.
#[napi]
pub enum ServerWasmLoading {
    Fetch,
    ModuleNode,
    Module,
}

impl From<ServerWasmLoading> for WasmLoading {
    fn from(target: ServerWasmLoading) -> Self {
        match target {
            ServerWasmLoading::Fetch => Self::Fetch,
            ServerWasmLoading::ModuleNode => Self::ModuleNode,
            ServerWasmLoading::Module => Self::Module,
        }
    }
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
server_result!(ServerChangeResult, bool);
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
