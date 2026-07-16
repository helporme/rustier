//! Driver for the Rust curse building. Generally ressembles the vitest plugin.
//!
//! I have no idea what api it should have nor I can plan it ahead, once I will
//! acome up with the api good enough to be used by the
//! vite/rollup/rolldown/tsdown/custom-cli -- I will refactor this. For now
//! this is a dump of everything.
//!
//! At the end, if rust-curse itself will be good enough, the driver can be
//! removed and js can just use rust-curse as lib.

use std::{
    collections::{BTreeMap, HashMap},
    error::Error as StdError,
    path::PathBuf,
    sync::Once,
};

use miette::Diagnostic;
use napi_derive::napi;

use thiserror::Error;
use tracing::{debug, info, trace};
use tracing_subscriber::{EnvFilter, fmt::time::ChronoLocal};

use crate::{
    codegen::{
        self,
        db::DbImpl,
        diag::{Diag, Theme},
        rustdoc::mod_::lower_mod,
    },
    sync::{SyncCrateOps, SyncTsTreeOpts, sync_crate, sync_ts_tree},
    tools::{self, node::BuildOutput as NodeBuildOutput, wasm::BuildOutput as WasmBuildOutput},
};

const DEFAULT_LOG_FILTER: &str = "rust_curse=info";
static SETUP_LOGGING: Once = Once::new();

fn setup_logging() {
    SETUP_LOGGING.call_once(|| {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));

        let timer = ChronoLocal::new("%H:%M:%S%.3f".to_string());

        let _ = tracing_subscriber::fmt()
            .compact()
            .with_timer(timer)
            .with_env_filter(filter)
            .try_init();
    });
}

fn napi_error(error: impl StdError) -> napi::Error {
    let mut reason = error.to_string();
    let mut previous_message = reason.clone();
    let mut source = error.source();
    let mut cause_index = 1;

    while let Some(error) = source {
        let message = error.to_string();
        if !message.is_empty() && message != previous_message {
            if cause_index == 1 {
                reason.push_str("\n\nCaused by:");
            }

            reason.push_str(&format!(
                "\n  {cause_index}: {}",
                message.replace('\n', "\n     ")
            ));
            cause_index += 1;
            previous_message = message;
        }
        source = error.source();
    }

    napi::Error::from_reason(reason)
}

#[napi]
pub enum WasmTarget {
    Web,
    NodeJs,
}

#[napi]
pub enum ProfileTarget {
    Wasm { target: WasmTarget },
    Node,
}

#[napi(object)]
pub struct ProfileOpts {
    pub target: ProfileTarget,
    pub release: bool,
}

#[napi(object)]
pub struct DriverOpts {
    /// path to the project Cargo.toml manifest
    pub manifest_path: String,
    /// path to .rust-curse/
    pub out_dir: String,
    /// profiles to build for
    pub profiles: HashMap<String, ProfileOpts>,
}

#[napi]
pub enum BuildArtifact {
    Wasm {
        wasm_abs_path: String,
        js_abs_path: String,
    },
    Node {
        js_abs_path: String,
        node_abs_path: String,
    },
}

#[napi(object)]
pub struct BuildOutput {
    pub per_profile: HashMap<String, ProfileBuildOutput>,
    pub diags: Vec<String>,
}

#[napi(object)]
pub struct ProfileBuildOutput {
    pub artifact: BuildArtifact,
    pub diags: Vec<String>,
}

enum BuildArtifacts {
    Wasm(WasmBuildOutput),
    Node(NodeBuildOutput),
}

impl ProfileOpts {
    fn api_kind(&self) -> codegen::api::ApiKind {
        match &self.target {
            ProfileTarget::Wasm { target } => codegen::api::ApiKind::Wasm {
                target: target.into(),
            },
            ProfileTarget::Node => codegen::api::ApiKind::Node,
        }
    }
}

impl From<&WasmTarget> for codegen::api::WasmTarget {
    fn from(target: &WasmTarget) -> Self {
        match target {
            WasmTarget::Web => Self::Web,
            WasmTarget::NodeJs => Self::Node,
        }
    }
}

impl From<&WasmTarget> for tools::wasm::WasmCompat {
    fn from(target: &WasmTarget) -> Self {
        match target {
            WasmTarget::Web => Self::Web,
            WasmTarget::NodeJs => Self::NodeJs,
        }
    }
}

impl From<&BuildArtifacts> for BuildArtifact {
    fn from(artifacts: &BuildArtifacts) -> Self {
        match artifacts {
            BuildArtifacts::Wasm(output) => BuildArtifact::Wasm {
                wasm_abs_path: output.wasm_abs_path.to_string_lossy().into_owned(),
                js_abs_path: output.js_abs_path.to_string_lossy().into_owned(),
            },
            BuildArtifacts::Node(output) => BuildArtifact::Node {
                js_abs_path: output.js_path.to_string_lossy().into_owned(),
                node_abs_path: output.node_path.to_string_lossy().into_owned(),
            },
        }
    }
}

#[napi]
pub struct Driver {
    opts: DriverOpts,
    db: DbImpl,
    profile_builds: HashMap<String, BuildArtifacts>,
    build_version: u64,
}

#[napi]
impl Driver {
    #[napi(constructor)]
    pub fn new(opts: DriverOpts) -> Self {
        setup_logging();

        debug!(
            manifest_path = %opts.manifest_path,
            out_dir = %opts.out_dir,
            profile_count = opts.profiles.len(),
            "creating rust-curse driver"
        );

        Self {
            opts,
            db: DbImpl::default(),
            profile_builds: HashMap::new(),
            build_version: 0,
        }
    }

    /// Installs rustdoc and profile-specific build tools required by configured
    /// profiles.
    #[napi]
    pub fn install_deps(&self) -> napi::Result<()> {
        self._install_deps().map_err(napi_error)
    }

    fn _install_deps(&self) -> Result<(), InstallDepsError> {
        let out_dir = PathBuf::from(&self.opts.out_dir);
        let tools_dir = out_dir.join("tools");
        let mut needs_wasm_tools = false;
        let mut needs_node_tools = false;

        for profile in self.opts.profiles.values() {
            match &profile.target {
                ProfileTarget::Wasm { .. } => needs_wasm_tools = true,
                ProfileTarget::Node => needs_node_tools = true,
            }
        }

        tools::rustdoc::ensure().map_err(|source| InstallDepsError::Rustdoc { source })?;

        if needs_wasm_tools {
            tools::wasm::ensure_wasm_target_installed()
                .map_err(|source| InstallDepsError::WasmTarget { source })?;
            tools::wasm::ensure_wasm_bindgen_cli_installed(
                tools::wasm::EnsureWasmBindgenCliInstalledOpts {
                    install_root_path: &tools_dir.join("wasm-bindgen"),
                },
            )
            .map_err(|source| InstallDepsError::WasmBindgen { source })?;
        }

        if needs_node_tools {
            tools::node::ensure_napi_cli_installed(tools::node::EnsureNapiCliInstalledOpts {
                install_root_path: &tools_dir.join("napi"),
            })
            .map_err(|source| InstallDepsError::Napi { source })?;
        }

        Ok(())
    }

    #[napi]
    pub fn build(&mut self, profile_names: Vec<String>) -> napi::Result<BuildOutput> {
        self._build(profile_names).map_err(napi_error)
    }

    #[tracing::instrument(name = "build", level = "debug", skip(self), fields(profile_names = ?profile_names))]
    fn _build(&mut self, profile_names: Vec<String>) -> Result<BuildOutput, BuildError> {
        info!(
            profile_count = profile_names.len(),
            build_version = self.build_version,
            "building profiles"
        );

        let manifest_path = PathBuf::from(&self.opts.manifest_path);
        let out_dir = PathBuf::from(&self.opts.out_dir);
        let tools_dir = out_dir.join("tools");
        let profile_crates_dir = out_dir.join("crates");
        let types_dir = out_dir.join("types");

        debug!(
            manifest_path = %manifest_path.display(),
            out_dir = %out_dir.display(),
            tools_dir = %tools_dir.display(),
            profile_crates_dir = %profile_crates_dir.display(),
            types_dir = %types_dir.display(),
            "resolved driver build paths"
        );

        let rustdoc = tools::rustdoc::run(&manifest_path)
            .map_err(|source| BuildError::GenerateRustdoc { source })?;

        let package = tools::workspace::get_package(&manifest_path)
            .map_err(|source| BuildError::GetPackage { source })?;

        let src_dir = package.source_package_path.join("src");
        let crates = BTreeMap::from([(
            package.source_crate_name,
            codegen::rustdoc::Crate {
                body: rustdoc,
                is_external: false,
                dir: package.source_package_path,
            },
        )]);
        codegen::rustdoc::RustdocCrates::set(&mut self.db, crates);

        let sync_ts_tree_diags = sync_ts_tree(SyncTsTreeOpts {
            our_dir: &types_dir,
            src_dir: &src_dir,
            db: &self.db,
        })
        .map_err(|source| BuildError::SyncTsTree { source })?;

        let mut diags = Vec::new();

        for diag in sync_ts_tree_diags.into_iter() {
            diags.push(
                diag.render(Theme::UnicodeNoColor)
                    .map_err(|source| BuildError::RenderDiagnostic { source })?,
            );
        }

        debug!(
            diag_count = diags.len(),
            "rendered shared codegen diagnostics"
        );

        let wasm_bindgen_tool_bin_path =
            tools::wasm::wasm_bindgen_cli_path(&tools_dir.join("wasm-bindgen"));
        let node_cli_path = tools::node::napi_cli_path(&tools_dir.join("napi"));

        let build_version = self.build_version;
        self.build_version += 1;

        let mut per_profile = HashMap::new();

        // todo: parallel builds
        for profile_name in profile_names {
            info!(
                profile_name = %profile_name,
                build_version,
                "building profile"
            );

            let profile = self.opts.profiles.get(&profile_name).ok_or_else(|| {
                BuildError::UnknownProfile {
                    profile_name: profile_name.clone(),
                }
            })?;
            let profile_crate_dir = profile_crates_dir.join(&profile_name);
            debug!(
                profile_name = %profile_name,
                profile_crate_dir = %profile_crate_dir.display(),
                "syncing profile crate"
            );

            codegen::api::Api::set(&mut self.db, profile.api_kind());
            sync_crate(SyncCrateOps {
                manifest_path: &manifest_path,
                out_dir: &profile_crate_dir,
                db: &self.db,
            })
            .map_err(|source| BuildError::SyncCrate { source })?;

            let mut profile_diags = Vec::new();
            for diag in codegen::bindings::compile_bindings::accumulated::<Diag>(&self.db) {
                profile_diags.push(
                    diag.render(Theme::UnicodeNoColor)
                        .map_err(|source| BuildError::RenderDiagnostic { source })?,
                );
            }
            debug!(
                profile_name = %profile_name,
                diag_count = profile_diags.len(),
                "rendered profile codegen diagnostics"
            );

            let output_root_dir = profile_crate_dir.join("output");
            let profile_out_dir =
                tools::build_versioning::get_versioned_dir(&output_root_dir, build_version);
            debug!(
                profile_name = %profile_name,
                output_root_dir = %output_root_dir.display(),
                profile_out_dir = %profile_out_dir.display(),
                "resolved profile output directories"
            );

            let artifacts = match &profile.target {
                ProfileTarget::Wasm { target } => BuildArtifacts::Wasm(
                    tools::wasm::build(tools::wasm::BuildOpts {
                        project_root_path: &profile_crate_dir,
                        out_dir_path: &profile_out_dir,
                        wasm_bindgen_tool_bin_path: &wasm_bindgen_tool_bin_path,
                        release: profile.release,
                        compat: target.into(),
                    })
                    .map_err(|source| BuildError::BuildWasm { source })?,
                ),
                ProfileTarget::Node => BuildArtifacts::Node(
                    tools::node::build(tools::node::BuildOpts {
                        project_root_path: &profile_crate_dir,
                        out_dir_path: &profile_out_dir,
                        node_cli_path: &node_cli_path,
                        release: profile.release,
                    })
                    .map_err(|source| BuildError::BuildNode { source })?,
                ),
            };

            tools::build_versioning::cleanup_old_build_versions(&output_root_dir, build_version)
                .map_err(|source| BuildError::CleanupBuildVersions { source })?;

            per_profile.insert(
                profile_name.clone(),
                ProfileBuildOutput {
                    artifact: BuildArtifact::from(&artifacts),
                    diags: profile_diags,
                },
            );
            self.profile_builds.insert(profile_name.clone(), artifacts);

            info!(profile_name = %profile_name, "built profile");
        }

        for diag in codegen::check::check(&self.db) {
            diags.push(
                Diag::from(diag.clone())
                    .render(Theme::UnicodeNoColor)
                    .map_err(|source| BuildError::RenderDiagnostic { source })?,
            );
        }

        debug!(
            diag_count = diags.len(),
            "rendered shared diagnostics after IR checks"
        );

        info!(
            profile_count = per_profile.len(),
            diag_count = diags.len(),
            "built profiles"
        );

        Ok(BuildOutput { per_profile, diags })
    }

    /// Resolves a Rust module import to JavaScript that re-exports generated
    /// bindings for the selected profile.
    #[napi]
    pub fn resolve(&mut self, profile_name: String, file_path: String) -> napi::Result<String> {
        self._resolve(&profile_name, file_path).map_err(napi_error)
    }

    #[tracing::instrument(
        name = "resolve",
        level = "debug",
        skip(self),
        fields(profile_name = %profile_name, file_path = %file_path)
    )]
    fn _resolve(&mut self, profile_name: &str, file_path: String) -> Result<String, ResolveError> {
        debug!(profile_name, file_path, "resolving Rust module import");

        let api_kind = self
            .opts
            .profiles
            .get(profile_name)
            .ok_or_else(|| ResolveError::UnknownProfile {
                profile_name: profile_name.to_string(),
            })?
            .api_kind();

        let artifacts = self.profile_builds.get(profile_name).ok_or_else(|| {
            ResolveError::MissingProfileBuildArtifacts {
                profile_name: profile_name.to_string(),
            }
        })?;
        let src_path = match artifacts {
            BuildArtifacts::Wasm(output) => output.js_abs_path.to_string_lossy().into_owned(),
            BuildArtifacts::Node(output) => output.js_path.to_string_lossy().into_owned(),
        };

        trace!(src_path, "resolved profile artifact source path");

        codegen::api::Api::set(&mut self.db, api_kind);
        codegen::js::JsCodegenPaths::set(&mut self.db, src_path);

        let module_path = PathBuf::from(&file_path).canonicalize().map_err(|source| {
            ResolveError::CanonicalizeModulePath {
                path: PathBuf::from(&file_path),
                source,
            }
        })?;

        debug!(module_path = %module_path.display(), "canonicalized Rust module path");

        // todo: build file map instaed
        for mod_id in codegen::rustdoc::mod_::all_mod_ids(&self.db) {
            let item = mod_id.item(&self.db);
            let Some(span) = &item.span else {
                continue;
            };
            let crate_ = mod_id.crate_(&self.db);
            let span_path = if span.filename.is_absolute() {
                span.filename.clone()
            } else {
                crate_.dir.join(&span.filename)
            };
            let Ok(span_path) = span_path.canonicalize() else {
                trace!("skipping rustdoc module with non-canonicalizable span path");
                continue;
            };

            trace!(span_path = %span_path.display(), "checking rustdoc module span path");

            if span_path == module_path {
                debug!(module_path = %module_path.display(), "resolved Rust module import");
                let mod_ = lower_mod(&self.db, *mod_id);
                let mod_id = mod_.id(&self.db);
                return Ok(codegen::js::mod_::compile_js_file(&self.db, mod_id).to_owned());
            }
        }

        Err(ResolveError::UnknownModule { path: module_path })
    }
}

#[derive(Error, Diagnostic, Debug)]
enum InstallDepsError {
    #[error("failed to install rustdoc prerequisites")]
    #[diagnostic(code(rust_curse::driver::install_rustdoc))]
    Rustdoc {
        #[source]
        source: tools::rustdoc::RustdocError,
    },

    #[error("failed to install wasm Rust target")]
    #[diagnostic(code(rust_curse::driver::install_wasm_target))]
    WasmTarget {
        #[source]
        source: tools::wasm::EnsureWasmTargetInstalledError,
    },

    #[error("failed to install wasm-bindgen CLI")]
    #[diagnostic(code(rust_curse::driver::install_wasm_bindgen))]
    WasmBindgen {
        #[source]
        source: tools::wasm::EnsureWasmBindgenCliInstalledError,
    },

    #[error("failed to install NAPI CLI")]
    #[diagnostic(code(rust_curse::driver::install_napi))]
    Napi {
        #[source]
        source: tools::node::EnsureNapiCliInstalledError,
    },
}

#[derive(Error, Diagnostic, Debug)]
enum ResolveError {
    #[error("unknown build profile `{profile_name}`")]
    #[diagnostic(code(rust_curse::driver::unknown_profile))]
    UnknownProfile { profile_name: String },

    #[error("profile `{profile_name}` has no build artifacts")]
    #[diagnostic(code(rust_curse::driver::missing_profile_build_artifacts))]
    MissingProfileBuildArtifacts { profile_name: String },

    #[error("failed to canonicalize Rust module path `{}`", path.display())]
    #[diagnostic(code(rust_curse::driver::canonicalize_module_path))]
    CanonicalizeModulePath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("no Rust module found for `{}`", path.display())]
    #[diagnostic(code(rust_curse::driver::unknown_module))]
    UnknownModule { path: PathBuf },
}

#[derive(Error, Diagnostic, Debug)]
enum BuildError {
    #[error("unknown build profile `{profile_name}`")]
    #[diagnostic(code(rust_curse::driver::unknown_profile))]
    UnknownProfile { profile_name: String },

    #[error("failed to inspect source package metadata")]
    #[diagnostic(code(rust_curse::driver::get_package))]
    GetPackage {
        #[source]
        source: tools::workspace::GetPackageErr,
    },

    #[error("failed to generate rustdoc JSON")]
    #[diagnostic(code(rust_curse::driver::generate_rustdoc))]
    GenerateRustdoc {
        #[source]
        source: tools::rustdoc::RustdocError,
    },

    #[error("failed to sync generated crate")]
    #[diagnostic(code(rust_curse::driver::sync_crate))]
    SyncCrate {
        #[source]
        source: crate::sync::SyncCrateError,
    },

    #[error("failed to sync TypeScript declaration tree")]
    #[diagnostic(code(rust_curse::driver::sync_ts_tree))]
    SyncTsTree {
        #[source]
        source: crate::sync::SyncTsTreeError,
    },

    #[error("failed to render codegen diagnostic")]
    #[diagnostic(code(rust_curse::driver::render_diagnostic))]
    RenderDiagnostic {
        #[source]
        source: std::fmt::Error,
    },

    #[error("failed to build wasm profile")]
    #[diagnostic(code(rust_curse::driver::build_wasm))]
    BuildWasm {
        #[source]
        source: tools::wasm::BuildError,
    },

    #[error("failed to build node profile")]
    #[diagnostic(code(rust_curse::driver::build_node))]
    BuildNode {
        #[source]
        source: tools::node::BuildError,
    },

    #[error("failed to clean up old build versions")]
    #[diagnostic(code(rust_curse::driver::cleanup_build_versions))]
    CleanupBuildVersions {
        #[source]
        source: tools::build_versioning::CleanupOldBuildVersionsError,
    },
}
