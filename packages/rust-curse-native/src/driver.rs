use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

use miette::Diagnostic;
use napi_derive::napi;
use thiserror::Error;

use crate::{
    codegen::{self, db::DbImpl},
    sync::{sync_crate, SyncCrateOps},
    tools::{self, node::BuildOutput as NodeBuildOutput, wasm::BuildOutput as WasmBuildOutput},
};

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
pub enum BuildOutput {
    Wasm {
        wasm_abs_path: String,
        js_abs_path: String,
    },
    Node {
        js_abs_path: String,
        node_abs_path: String,
    },
}

enum BuildArtifacts {
    Wasm(WasmBuildOutput),
    Node(NodeBuildOutput),
}

impl ProfileOpts {
    fn api(&self) -> codegen::bindings::Api {
        match &self.target {
            ProfileTarget::Wasm { .. } => codegen::bindings::Api::Wasm,
            ProfileTarget::Node => codegen::bindings::Api::Node,
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

impl From<&BuildArtifacts> for BuildOutput {
    fn from(artifacts: &BuildArtifacts) -> Self {
        match artifacts {
            BuildArtifacts::Wasm(output) => BuildOutput::Wasm {
                wasm_abs_path: output.wasm_abs_path.to_string_lossy().into_owned(),
                js_abs_path: output.js_abs_path.to_string_lossy().into_owned(),
            },
            BuildArtifacts::Node(output) => BuildOutput::Node {
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
}

#[napi]
impl Driver {
    #[napi(constructor)]
    pub fn new(opts: DriverOpts) -> Self {
        Self {
            opts,
            db: DbImpl::default(),
            profile_builds: HashMap::new(),
        }
    }

    /// Installs rustdoc and profile-specific build tools required by configured
    /// profiles.
    #[napi]
    pub fn install_deps(&self) -> napi::Result<()> {
        self._install_deps()
            .map_err(|error| napi::Error::from_reason(error.to_string()))
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

        tools::rustdoc::ensure().map_err(|source| InstallDepsError::EnsureRustdoc { source })?;

        if needs_wasm_tools {
            tools::wasm::ensure_wasm_target_installed()
                .map_err(|source| InstallDepsError::EnsureWasmTarget { source })?;
            tools::wasm::ensure_wasm_bindgen_cli_installed(
                tools::wasm::EnsureWasmBindgenCliInstalledOpts {
                    install_root_path: &tools_dir.join("wasm-bindgen"),
                },
            )
            .map_err(|source| InstallDepsError::EnsureWasmBindgen { source })?;
        }

        if needs_node_tools {
            tools::node::ensure_napi_cli_installed(tools::node::EnsureNapiCliInstalledOpts {
                install_root_path: &tools_dir.join("napi"),
            })
            .map_err(|source| InstallDepsError::EnsureNapi { source })?;
        }

        Ok(())
    }

    #[napi]
    pub fn build(
        &mut self,
        profile_names: Vec<String>,
    ) -> napi::Result<HashMap<String, BuildOutput>> {
        self._build(profile_names)
            .map_err(|error| napi::Error::from_reason(error.to_string()))
    }

    fn _build(
        &mut self,
        profile_names: Vec<String>,
    ) -> Result<HashMap<String, BuildOutput>, BuildError> {
        let manifest_path = PathBuf::from(&self.opts.manifest_path);
        let out_dir = PathBuf::from(&self.opts.out_dir);
        let tools_dir = out_dir.join("tools");
        let profile_crates_dir = out_dir.join("crates");

        let rustdoc = tools::rustdoc::run(&manifest_path)
            .map_err(|source| BuildError::GenerateRustdoc { source })?;

        let package = tools::workspace::get_package(&manifest_path)
            .map_err(|source| BuildError::GetPackage { source })?;

        codegen::rustdoc::Input::new(
            &self.db,
            BTreeMap::from([(
                package.source_crate_name,
                codegen::rustdoc::Crate {
                    body: rustdoc,
                    is_external: false,
                    dir: package.source_package_path,
                },
            )]),
        );

        let wasm_bindgen_tool_bin_path =
            tools::wasm::wasm_bindgen_cli_path(&tools_dir.join("wasm-bindgen"));
        let node_cli_path = tools::node::napi_cli_path(&tools_dir.join("napi"));

        let mut outputs = HashMap::new();

        for profile_name in profile_names {
            let profile = self.opts.profiles.get(&profile_name).ok_or_else(|| {
                BuildError::UnknownProfile {
                    profile_name: profile_name.clone(),
                }
            })?;
            let profile_crate_dir = profile_crates_dir.join(&profile_name);
            let build_required = sync_crate(SyncCrateOps {
                manifest_path: &manifest_path,
                out_dir: &profile_crate_dir,
                api: profile.api(),
                db: &self.db,
            })
            .map_err(|source| BuildError::SyncCrate { source })?
                || !self.profile_builds.contains_key(&profile_name);

            if build_required {
                let profile_out_dir = profile_crate_dir.join("output");
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

                outputs.insert(profile_name.clone(), BuildOutput::from(&artifacts));
                self.profile_builds.insert(profile_name, artifacts);
                continue;
            }

            let artifacts = self.profile_builds.get(&profile_name).ok_or_else(|| {
                BuildError::MissingProfileBuildArtifacts {
                    profile_name: profile_name.clone(),
                }
            })?;
            outputs.insert(profile_name, artifacts.into());
        }

        Ok(outputs)
    }
}

#[derive(Error, Diagnostic, Debug)]
enum InstallDepsError {
    #[error("failed to ensure rustdoc prerequisites")]
    #[diagnostic(code(rust_curse::driver::ensure_rustdoc))]
    EnsureRustdoc {
        #[source]
        source: tools::rustdoc::RustdocError,
    },

    #[error("failed to ensure wasm Rust target")]
    #[diagnostic(code(rust_curse::driver::ensure_wasm_target))]
    EnsureWasmTarget {
        #[source]
        source: tools::wasm::EnsureWasmTargetInstalledError,
    },

    #[error("failed to ensure wasm-bindgen CLI")]
    #[diagnostic(code(rust_curse::driver::ensure_wasm_bindgen))]
    EnsureWasmBindgen {
        #[source]
        source: tools::wasm::EnsureWasmBindgenCliInstalledError,
    },

    #[error("failed to ensure NAPI CLI")]
    #[diagnostic(code(rust_curse::driver::ensure_napi))]
    EnsureNapi {
        #[source]
        source: tools::node::EnsureNapiCliInstalledError,
    },
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

    #[error("profile `{profile_name}` has no build artifacts")]
    #[diagnostic(code(rust_curse::driver::missing_profile_build_artifacts))]
    MissingProfileBuildArtifacts { profile_name: String },
}
