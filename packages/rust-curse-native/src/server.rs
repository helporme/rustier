//! Long-lived build server used by JavaScript bundler integrations.

mod fs;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use napi_derive::napi;

use crate::{
    compiler::{
        CompileJsOpts, CompileShimOpts, Compiler, CompilerDiagnostic, FilePath, Platform, Theme,
        WorkspaceChange,
    },
    sync::{
        SyncCrateOpts, SyncTsTreeOpts, sync_crate, sync_ts_tree, workspace::PopulateWorkspaceOpts,
    },
    tools,
};

#[derive(Clone)]
#[napi]
pub enum WasmTarget {
    Web,
    NodeJs,
}

#[derive(Clone)]
#[napi]
pub enum Target {
    Wasm { target: WasmTarget },
    Node,
}

impl Target {
    fn platform(&self) -> Platform {
        match self {
            Self::Wasm {
                target: WasmTarget::Web,
            } => Platform::Wasm {
                target: crate::compiler::WasmEnv::Web,
            },
            Self::Wasm {
                target: WasmTarget::NodeJs,
            } => Platform::Wasm {
                target: crate::compiler::WasmEnv::Node,
            },
            Self::Node => Platform::Node,
        }
    }
}

#[derive(Clone)]
#[napi(object)]
pub struct ProfileOpts {
    pub target: Target,
    pub release: bool,
}

#[napi(object)]
pub struct ServerOpts {
    pub manifest_path: String,
    pub out_dir: String,
    pub profiles: HashMap<String, ProfileOpts>,
    pub included: Vec<String>,
    pub excluded: Vec<String>,
    pub include_dep_crates: Vec<String>,
    pub exclude_dep_crates: Vec<String>,
}

#[napi(object)]
pub struct FileChange {
    pub file_path: String,
    pub contents: Option<String>,
}

#[derive(Clone)]
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

#[derive(Clone)]
#[napi(object)]
pub struct ProfileBuildOutput {
    pub artifact: BuildArtifact,
    pub diags: Vec<String>,
}

#[napi(object)]
pub struct BuildOutput {
    pub per_profile: HashMap<String, ProfileBuildOutput>,
}

impl BuildArtifact {
    fn js_path(&self) -> &str {
        match self {
            Self::Wasm { js_abs_path, .. } | Self::Node { js_abs_path, .. } => js_abs_path,
        }
    }
}

#[derive(Default)]
struct ServerChanges {
    files: HashMap<FilePath, Option<String>>,
    reload_workspace: bool,
    dirty_profiles: HashSet<String>,
}

struct ProfileBuild {
    output: ProfileBuildOutput,
    next_version: u64,
}

#[napi]
pub struct Server {
    opts: ServerOpts,
    compiler: Compiler,
    pending: Option<ServerChanges>,
    profile_builds: HashMap<String, ProfileBuild>,
}

#[napi]
impl Server {
    #[napi(constructor)]
    pub fn new(opts: ServerOpts) -> Self {
        Self {
            pending: Some(ServerChanges {
                reload_workspace: true,
                dirty_profiles: opts.profiles.keys().cloned().collect(),
                ..Default::default()
            }),
            opts,
            compiler: Compiler::new(),
            profile_builds: HashMap::new(),
        }
    }

    #[napi]
    pub fn install_deps(&self) -> napi::Result<()> {
        let tools_dir = PathBuf::from(&self.opts.out_dir).join("tools");
        let needs_wasm = self
            .opts
            .profiles
            .values()
            .any(|profile| matches!(&profile.target, Target::Wasm { .. }));
        let needs_node = self
            .opts
            .profiles
            .values()
            .any(|profile| matches!(&profile.target, Target::Node));

        if needs_wasm {
            tools::wasm::ensure_wasm_target_installed().map_err(napi_error)?;
            tools::wasm::ensure_wasm_bindgen_cli_installed(
                tools::wasm::EnsureWasmBindgenCliInstalledOpts {
                    install_root_path: &tools_dir.join("wasm-bindgen"),
                },
            )
            .map_err(napi_error)?;
        }
        if needs_node {
            tools::node::ensure_napi_cli_installed(tools::node::EnsureNapiCliInstalledOpts {
                install_root_path: &tools_dir.join("napi"),
            })
            .map_err(napi_error)?;
        }

        Ok(())
    }

    #[napi]
    pub fn update(&mut self, changes: Vec<FileChange>) -> napi::Result<()> {
        let mut changed = false;
        for change in changes {
            let path = PathBuf::from(&change.file_path);
            if path.extension().is_some_and(|extension| extension == "rs") {
                self.pending.get_or_insert_default().files.insert(
                    FilePath::try_from(path.as_path()).map_err(napi_error)?,
                    change.contents,
                );
                changed = true;
            } else if path == Path::new(&self.opts.manifest_path) {
                self.pending.get_or_insert_default().reload_workspace = true;
                changed = true;
            }
        }
        if changed {
            self.pending
                .get_or_insert_default()
                .dirty_profiles
                .extend(self.opts.profiles.keys().cloned());
        }
        Ok(())
    }

    #[napi]
    pub fn build(&mut self, profile_names: Vec<String>) -> napi::Result<BuildOutput> {
        self.validate_profiles(&profile_names)?;
        let manifest_path = PathBuf::from(&self.opts.manifest_path);
        let out_dir = PathBuf::from(&self.opts.out_dir);
        self.apply_workspace_changes(&manifest_path, &out_dir)?;

        let mut per_profile = HashMap::new();
        for profile_name in profile_names {
            let output = self.build_profile(&profile_name, &manifest_path, &out_dir)?;
            per_profile.insert(profile_name, output);
        }
        self.clear_pending_if_empty();
        Ok(BuildOutput { per_profile })
    }

    #[napi]
    pub fn load(&mut self, profile_name: String, file_path: String) -> napi::Result<String> {
        if !self.opts.profiles.contains_key(&profile_name) {
            return Err(napi::Error::from_reason(format!(
                "unknown build profile `{profile_name}`"
            )));
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.dirty_profiles.contains(&profile_name))
        {
            return Err(napi::Error::from_reason(format!(
                "profile `{profile_name}` must be built"
            )));
        }
        let build = self.profile_builds.get(&profile_name).ok_or_else(|| {
            napi::Error::from_reason(format!("profile `{profile_name}` has no build artifacts"))
        })?;

        let path = PathBuf::from(file_path)
            .canonicalize()
            .map_err(napi_error)?;

        self.compiler
            .compile_js(
                FilePath::try_from(path.as_path()).map_err(napi_error)?,
                CompileJsOpts {
                    ffi_js_file: FilePath::new_real(build.output.artifact.js_path())
                        .map_err(napi_error)?,
                },
            )
            .ok_or_else(|| napi::Error::from_reason("Rust module is not part of the workspace"))
    }
}

impl Server {
    fn validate_profiles(&self, profile_names: &[String]) -> napi::Result<()> {
        for profile_name in profile_names {
            if !self.opts.profiles.contains_key(profile_name) {
                return Err(napi::Error::from_reason(format!(
                    "unknown build profile `{profile_name}`"
                )));
            }
        }
        Ok(())
    }

    fn apply_workspace_changes(
        &mut self,
        manifest_path: &Path,
        out_dir: &Path,
    ) -> napi::Result<()> {
        let Some(pending) = &self.pending else {
            return Ok(());
        };
        if !pending.reload_workspace && pending.files.is_empty() {
            return Ok(());
        }

        let reload_workspace = pending.reload_workspace;
        let files = pending
            .files
            .iter()
            .map(|(path, contents)| (path.clone(), contents.clone()))
            .collect::<Vec<_>>();

        if reload_workspace {
            let mut compiler = Compiler::new();
            let opts = PopulateWorkspaceOpts {
                manifest_path,
                included: self.opts.included.clone(),
                excluded: self.opts.excluded.clone(),
                include_dep_crates: self.opts.include_dep_crates.clone(),
                exclude_dep_crates: self.opts.exclude_dep_crates.clone(),
                include_std: true,
            };
            let mut change = WorkspaceChange::new();
            change.populate_from_fs(opts).map_err(napi_error)?;
            compiler.change(change).map_err(napi_error)?;
            self.compiler = compiler;
        } else {
            let mut change = WorkspaceChange::new();
            for (path, contents) in files {
                change.set_local_file(path, contents);
            }
            self.compiler.change(change).map_err(napi_error)?;
        }

        sync_ts_tree(SyncTsTreeOpts {
            out_dir: &out_dir.join("types"),
            package_dir: manifest_path
                .parent()
                .ok_or_else(|| napi::Error::from_reason("manifest path has no parent"))?,
            tree: self.compiler.compile_ts_tree(),
        })
        .map_err(napi_error)?;

        let pending = self
            .pending
            .as_mut()
            .expect("workspace changes are still pending");
        pending.files.clear();
        pending.reload_workspace = false;
        Ok(())
    }

    fn build_profile(
        &mut self,
        profile_name: &str,
        manifest_path: &Path,
        out_dir: &Path,
    ) -> napi::Result<ProfileBuildOutput> {
        let is_dirty = self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.dirty_profiles.contains(profile_name));
        if !is_dirty {
            return self
                .profile_builds
                .get(profile_name)
                .map(|build| build.output.clone())
                .ok_or_else(|| {
                    napi::Error::from_reason(format!(
                        "profile `{profile_name}` has no build artifacts"
                    ))
                });
        }

        let profile = self
            .opts
            .profiles
            .get(profile_name)
            .expect("profile names were validated")
            .clone();
        let platform = profile.target.platform();
        let compilation = self.compiler.compile_shim(CompileShimOpts { platform });
        let diags = render_diags(compilation.diagnostics)?;
        let crate_dir = out_dir.join("crates").join(profile_name);
        sync_crate(SyncCrateOpts {
            manifest_path,
            out_dir: &crate_dir,
            platform,
            shim: &compilation.shim,
        })
        .map_err(napi_error)?;

        let build_version = self
            .profile_builds
            .get(profile_name)
            .map_or(0, |build| build.next_version);
        let output_root = crate_dir.join("output");
        let output_dir = tools::build_versioning::get_versioned_dir(&output_root, build_version);
        let artifact = Self::build_profile_artifact(
            &profile,
            &crate_dir,
            &output_dir,
            &out_dir.join("tools"),
        )?;
        tools::build_versioning::cleanup_old_build_versions(&output_root, build_version)
            .map_err(napi_error)?;

        let output = ProfileBuildOutput { artifact, diags };
        self.profile_builds.insert(
            profile_name.to_owned(),
            ProfileBuild {
                output: output.clone(),
                next_version: build_version + 1,
            },
        );
        self.pending
            .as_mut()
            .expect("dirty profiles are pending")
            .dirty_profiles
            .remove(profile_name);
        Ok(output)
    }

    fn build_profile_artifact(
        profile: &ProfileOpts,
        crate_dir: &Path,
        output_dir: &Path,
        tools_dir: &Path,
    ) -> napi::Result<BuildArtifact> {
        match &profile.target {
            Target::Wasm { target } => {
                let compat = match target {
                    WasmTarget::Web => tools::wasm::WasmCompat::Web,
                    WasmTarget::NodeJs => tools::wasm::WasmCompat::NodeJs,
                };
                let output = tools::wasm::build(tools::wasm::BuildOpts {
                    project_root_path: crate_dir,
                    out_dir_path: output_dir,
                    wasm_bindgen_tool_bin_path: &tools::wasm::wasm_bindgen_cli_path(
                        &tools_dir.join("wasm-bindgen"),
                    ),
                    release: profile.release,
                    compat,
                })
                .map_err(napi_error)?;
                Ok(BuildArtifact::Wasm {
                    wasm_abs_path: output.wasm_abs_path.to_string_lossy().into_owned(),
                    js_abs_path: output.js_abs_path.to_string_lossy().into_owned(),
                })
            }
            Target::Node => {
                let output = tools::node::build(tools::node::BuildOpts {
                    project_root_path: crate_dir,
                    out_dir_path: output_dir,
                    node_cli_path: &tools::node::napi_cli_path(&tools_dir.join("napi")),
                    release: profile.release,
                })
                .map_err(napi_error)?;
                Ok(BuildArtifact::Node {
                    js_abs_path: output.js_path.to_string_lossy().into_owned(),
                    node_abs_path: output.node_path.to_string_lossy().into_owned(),
                })
            }
        }
    }

    fn clear_pending_if_empty(&mut self) {
        if self.pending.as_ref().is_some_and(|pending| {
            !pending.reload_workspace
                && pending.files.is_empty()
                && pending.dirty_profiles.is_empty()
        }) {
            self.pending = None;
        }
    }
}

fn render_diags(diags: Vec<CompilerDiagnostic>) -> napi::Result<Vec<String>> {
    diags
        .into_iter()
        .map(|diag| diag.render(Theme::UnicodeNoColor).map_err(napi_error))
        .collect()
}

fn napi_error(error: impl std::error::Error) -> napi::Error {
    let mut reason = error.to_string();
    let mut source = error.source();

    if source.is_some() {
        reason.push_str("\n\nCaused by:");
    }
    let mut index = 1;
    while let Some(cause) = source {
        reason.push_str(&format!("\n  {index}: {cause}"));
        source = cause.source();
        index += 1;
    }

    napi::Error::from_reason(reason)
}
