use core::str;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{anyhow, bail, Context};
#[cfg(feature = "napi-binding")]
use napi_derive::napi;

#[cfg(feature = "napi-binding")]
use crate::builder::napi::{NapiBuildFn, NapiBuildOpts, NapiBuilder};
use crate::ir::{Id, Item, IR};
use crate::orphans::OrphanScanOpts;
use crate::sync::write_orphan_tree::WriteOrphanTreeOpts;
use crate::sync::Syncer;
use crate::tree_shake::{self, TreeShakeOpts};
use crate::{builder, common, orphans, rustdoc};

#[cfg_attr(feature = "napi-binding", napi(discriminant_case = "kebab-case"))]
pub enum ProfileBuildTarget {
    Wasm {},
    Node {},
}

#[derive(Debug)]
#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct FileFilter {
    /// Glob patterns to include.
    pub include: Option<Vec<String>>,
    /// Glob patterns to exclude.
    pub exclude: Option<Vec<String>>,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct ProfileRustOpts {
    /// Profile build target.
    pub target: ProfileBuildTarget,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct ProfileOpts {
    /// Profile name.
    ///
    /// Can contain only alphanumeric characters and hyphens.
    pub name: String,

    /// Profile rust related options.
    pub rust: ProfileRustOpts,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct RustOpts {
    /// Directory to discover rust orphan modules, relative to project root.
    /// Usually src/.
    pub modules_root_dir: String,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct JsOpts {
    /// Filter for js-like (ts/tsx/jsx/svelte/etc.) files that are
    /// included/excluded from the build.
    pub filter: FileFilter,
    /// Generate ts types for rust modules?
    pub typescript: bool,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct RustCurseOpts {
    /// Absolute path to the project root directory.
    pub project_root: String,
    /// Output dir for placing generated files, relative to project root.
    pub out_dir: String,
    /// Global rust related options.
    pub rust: RustOpts,
    /// Global js related options.
    pub js: JsOpts,
    /// Build profiles.
    pub profiles: Vec<ProfileOpts>,
}

impl RustCurseOpts {
    pub fn validate(&self) -> Result<(), anyhow::Error> {
        for profile in &self.profiles {
            self.validate_profile_name(&profile.name)?;
        }
        Ok(())
    }

    fn validate_profile_name(&self, name: &str) -> Result<(), anyhow::Error> {
        if name.is_empty() {
            bail!("profile name cannot be empty");
        }

        if name.len() > 255 {
            bail!("profile name is too long (max 255 characters)");
        }

        if let Some(c) = name
            .chars()
            .find(|c| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_'))
        {
            bail!(
                "profile name can only contain ASCII letters and digits, found {:?}",
                c
            );
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct BuildCache {
    ir: IR,
    exposed: HashSet<Id>,
    artifacts: HashMap<String, ProfileBuildCache>,
}

#[derive(Debug, Clone)]
pub enum ProfileBuildCache {
    Wasm(builder::wasm::WasmBuildOutput),
    Node(builder::napi::NapiBuildOutput),
}

impl BuildCache {
    pub fn mint_build_output(&self) -> BuildOutput {
        BuildOutput {
            artifacts: self
                .artifacts
                .iter()
                .map(|(name, cache)| {
                    (
                        name.clone(),
                        match cache {
                            ProfileBuildCache::Wasm(wasm_output) => ProfileBuildOutput::Wasm {
                                wasm_abs_path: wasm_output
                                    .wasm_output_abs_path
                                    .to_string_lossy()
                                    .into_owned(),
                            },
                            ProfileBuildCache::Node(node_output) => ProfileBuildOutput::Node {
                                node_paths: node_output.node_output_abs_paths.clone(),
                            },
                        },
                    )
                })
                .collect(),
        }
    }
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct BuildOutput {
    pub artifacts: HashMap<String, ProfileBuildOutput>,
}

#[cfg_attr(feature = "napi-binding", napi(discriminant_case = "kebab-case"))]
#[derive(Debug, Clone)]
pub enum ProfileBuildOutput {
    Wasm { wasm_abs_path: String },
    Node { node_paths: Vec<String> },
}

/// Per-profile artifact paths produced by a wasm-targeting build.
#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct WasmProfileArtifact {
    pub profile_name: String,
    /// Absolute path to the wasm-bindgen-generated `package.js`.
    pub js_abs_path: String,
    /// Absolute path to the compiled `package_bg.wasm`.
    pub wasm_abs_path: String,
}

#[cfg_attr(feature = "napi-binding", napi)]
pub struct RustCurse {
    /// Project root directory absolute path
    project_root_abs_path: PathBuf,
    /// Modules root directory relative to project root (e.g. "./src")
    modules_root_rel_path: PathBuf,
    /// Output directory absolute path (e.g. "/path/to/project/.rust-curse")
    out_dir_abs_path: PathBuf,
    /// Build output directory absolute path (e.g.
    /// "/path/to/project/.rust-curse/output")
    build_out_dir_abs_path: PathBuf,
    /// Passed options
    options: RustCurseOpts,
    /// Last successful build artifacts
    last_build: Mutex<Option<BuildCache>>,
    /// Napi builder
    napi_builder: NapiBuilder,
}

#[cfg_attr(feature = "napi-binding", napi)]
impl RustCurse {
    #[cfg(feature = "napi-binding")]
    #[cfg_attr(feature = "napi-binding", napi(constructor))]
    pub fn new(options: RustCurseOpts, napi_build_fn: NapiBuildFn) -> Result<Self, napi::JsError> {
        options
            .validate()
            .map_err(|e| napi::Error::new(napi::Status::InvalidArg, format!("{:#}", e)))?;

        let project_root_abs_path = PathBuf::from(&options.project_root);
        let modules_root_rel_path = PathBuf::from(&options.rust.modules_root_dir);
        let out_dir_abs_path = project_root_abs_path.join(&options.out_dir);
        let build_out_dir_abs_path = out_dir_abs_path.join("output");

        Ok(Self {
            project_root_abs_path,
            modules_root_rel_path,
            out_dir_abs_path,
            build_out_dir_abs_path,
            options,
            napi_builder: napi_build_fn.into(),
            last_build: Mutex::new(None),
        })
    }

    #[cfg(feature = "napi-binding")]
    #[napi]
    pub async fn build(&self, profile_names: Vec<String>) -> napi::Result<BuildOutput> {
        self._build(&profile_names)
            .await
            .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("{:#}", e)))
    }

    #[cfg(feature = "napi-binding")]
    #[napi]
    pub fn resolve(&self, profile_name: String, rs_rel_path: String) -> napi::Result<String> {
        self._resolve(&profile_name, &rs_rel_path)
            .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("{:#}", e)))
    }

    async fn _build(&self, profile_names: &[String]) -> anyhow::Result<BuildOutput> {
        let orphan_modules = orphans::scan(OrphanScanOpts {
            project_root_abs_path: &self.project_root_abs_path,
            src_rel_path: &self.modules_root_rel_path,
            rs_filter: None,
        })?;

        let mut sync = Syncer::new(
            self.project_root_abs_path.clone(),
            PathBuf::from(&self.options.out_dir),
        );

        sync.write_wired_cargo_toml()
            .context("sync: wired Cargo.toml")?;
        sync.write_rustdoc(&orphan_modules)
            .context("sync: rustdoc")?;

        let rustdoc = rustdoc::gen(&self.project_root_abs_path)?;
        let ir = IR::try_from(&rustdoc).context("build ir")?;

        let exposed = tree_shake::shake(
            &ir,
            TreeShakeOpts {
                project_root_abs_path: &self.project_root_abs_path,
                src_rel_path: &self.modules_root_rel_path,

                // todo: avoid it after self-host
                js_filter: &common::FileFilter {
                    include: self.options.js.filter.include.clone(),
                    exclude: self.options.js.filter.exclude.clone(),
                },
            },
        )
        .with_context(|| "tree_shake")?;

        sync.write_orphan_tree(WriteOrphanTreeOpts {
            ir: &ir,
            exposed: &exposed,
        })
        .context("sync: orphan_tree")?;

        let profiles = self.get_profiles(profile_names)?;
        for profile in &profiles {
            match profile.rust.target {
                ProfileBuildTarget::Node {} => sync
                    .write_napi(&profile.name, ir.print_napi(&exposed))
                    .with_context(|| format!("sync: napi profile '{}'", profile.name))?,
                ProfileBuildTarget::Wasm {} => sync
                    .write_wasm(&profile.name, ir.print_wasm(&exposed))
                    .with_context(|| format!("sync: wasm profile '{}'", profile.name))?,
            }
        }

        if self.options.js.typescript {
            sync.write_ts(&ir, &self.modules_root_rel_path)
                .context("sync: ts")?;
        }

        let wasm_bindgen_tool_bin_abs_path: Option<PathBuf> = profiles
            .iter()
            .any(|p| matches!(p.rust.target, ProfileBuildTarget::Wasm {}))
            .then(|| {
                builder::wasm::ensure_wasm_target_installed()?;
                builder::wasm::ensure_bindgen_tool_installed(
                    &self.out_dir_abs_path.join("tools/wasm-bindgen-cli"),
                )
            })
            .transpose()?;

        let mut artifacts: HashMap<String, ProfileBuildCache> = HashMap::new();

        for profile in &profiles {
            match profile.rust.target {
                ProfileBuildTarget::Wasm {} => {
                    let bin = wasm_bindgen_tool_bin_abs_path.as_deref().expect(
                        "wasm-bindgen bin path was provisioned above for any wasm-targeting profile",
                    );
                    let output = builder::wasm::build(builder::wasm::WasmBuildOpts {
                        project_root_abs_path: &self.project_root_abs_path,
                        out_dir_abs_path: &self.build_out_dir_abs_path,
                        wasm_bindgen_tool_bin_abs_path: bin,
                        profile_name: &profile.name,
                        release: false,
                    })
                    .with_context(|| format!("builder: wasm profile '{}'", profile.name))?;

                    artifacts.insert(profile.name.clone(), ProfileBuildCache::Wasm(output));
                }
                ProfileBuildTarget::Node {} => {
                    let output = self
                        .napi_builder
                        .build(NapiBuildOpts {
                            release: false,
                            profile: &profile.name,
                            output_abs_dir: &self.build_out_dir_abs_path,
                            project_root_abs_path: &self.project_root_abs_path,
                        })
                        .await?;
                    artifacts.insert(profile.name.clone(), ProfileBuildCache::Node(output));
                }
            }
        }

        let build_cache = BuildCache {
            ir,
            exposed,
            artifacts,
        };
        let build_output = build_cache.mint_build_output();

        *self.last_build.lock().unwrap() = Some(build_cache);

        Ok(build_output)
    }

    fn _resolve(&self, profile_name: &str, path: &str) -> anyhow::Result<String> {
        let last_build = self.last_build.lock().unwrap();
        let build_output = last_build.as_ref().ok_or(anyhow!(
            "resolve cannot be called before build, please call build first."
        ))?;

        let artifact = build_output
            .artifacts
            .get(profile_name)
            .ok_or_else(|| anyhow!("unknown profile '{profile_name}'"))?;

        let path_buf = PathBuf::from(path);
        let mid = Id(build_output
            .ir
            .items
            .iter()
            .enumerate()
            .find(
                |(_, i)| matches!(i, Item::Mod(def) if def.span.as_ref().map(|s| s.filename.as_path()) == Some(path_buf.as_path())),
            )
            .ok_or(anyhow!("cannot resolve the module '{}'", path))?
            .0 as u32);

        let js_glue = match artifact {
            ProfileBuildCache::Wasm(wasm) => {
                let js_output_abs_path = wasm.js_output_abs_path.to_string_lossy().to_string();
                build_output
                    .ir
                    .print_wasm_js(mid, js_output_abs_path, &build_output.exposed)
            }
            ProfileBuildCache::Node(node) => {
                let js_output_abs_path = node
                    .js_output_abs_path
                    .as_ref()
                    .ok_or(anyhow!("no JS artifact found in napi build output"))?
                    .clone();

                build_output
                    .ir
                    .print_napi_js(mid, js_output_abs_path, &build_output.exposed)
            }
        };

        Ok(js_glue)
    }

    fn get_profiles(&self, names: &[String]) -> Result<Vec<&ProfileOpts>, anyhow::Error> {
        let profiles = names
            .iter()
            .map(|name| {
                self.options
                    .profiles
                    .iter()
                    .find(|p| &p.name == name)
                    .ok_or_else(|| anyhow::anyhow!("unknown profile '{name}'"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(profiles)
    }
}
