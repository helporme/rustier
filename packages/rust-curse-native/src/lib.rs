pub mod builder;
pub mod common; // todo: should be private
pub mod ir;
pub mod orphans;
pub mod rustdoc;
pub mod sync;
pub mod tree_shake;

use std::path::PathBuf;

use anyhow::{bail, Context};
#[cfg(feature = "napi-binding")]
use napi_derive::napi;

use crate::builder::wasm::{build_wasm, BuildWasmOpts};
use crate::ir::IR;
use crate::orphans::OrphanScanOpts;
use crate::sync::write_orphan_tree::WriteOrphanTreeOpts;
use crate::sync::Syncer;
use crate::tree_shake::TreeShakeOpts;

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

#[cfg_attr(feature = "napi-binding", napi)]
pub struct RustCurse {
    options: RustCurseOpts,
}

#[cfg_attr(feature = "napi-binding", napi)]
impl RustCurse {
    #[cfg(feature = "napi-binding")]
    #[cfg_attr(feature = "napi-binding", napi(constructor))]
    pub fn new(options: RustCurseOpts) -> Result<Self, napi::JsError> {
        options
            .validate()
            .map_err(|e| napi::Error::new(napi::Status::InvalidArg, format!("{:#}", e)))?;

        Ok(Self { options })
    }

    #[cfg(feature = "napi-binding")]
    #[napi]
    pub fn build(&mut self, profile_names: Vec<String>) -> napi::Result<()> {
        self.try_build(&profile_names)
            .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("{:#}", e)))
    }

    fn try_build(&mut self, profile_names: &[String]) -> anyhow::Result<()> {
        let project_root_abs_path = PathBuf::from(&self.options.project_root);
        let modules_root_rel_path = PathBuf::from(&self.options.rust.modules_root_dir);

        let orphan_modules = orphans::scan(OrphanScanOpts {
            project_root_abs_path: &project_root_abs_path,
            src_rel_path: &modules_root_rel_path,
            rs_filter: None,
        })?;

        let mut sync = Syncer::new(
            project_root_abs_path.clone(),
            PathBuf::from(&self.options.out_dir),
        );

        sync.write_wired_cargo_toml()
            .context("sync: wired Cargo.toml")?;
        sync.write_rustdoc(&orphan_modules)
            .context("sync: rustdoc")?;

        let rustdoc = rustdoc::gen(&project_root_abs_path)?;
        let ir = IR::try_from(&rustdoc).context("build ir")?;

        let exposed_mod_items = tree_shake::shake(
            &ir,
            TreeShakeOpts {
                project_root_abs_path: &project_root_abs_path,
                src_rel_path: &modules_root_rel_path,
                js_filter: &self.options.js.filter,
            },
        )
        .with_context(|| "tree_shake")?;

        sync.write_orphan_tree(WriteOrphanTreeOpts {
            modules: &orphan_modules,
            ir: &ir,
            exposed: &exposed_mod_items,
        })
        .context("sync: orphan_tree")?;

        let profiles = self.get_profiles(profile_names)?;
        for profile in &profiles {
            match profile.rust.target {
                ProfileBuildTarget::Node {} => sync
                    .write_napi(&profile.name, ir.print_napi(&exposed_mod_items))
                    .with_context(|| format!("sync: napi profile '{}'", profile.name))?,
                ProfileBuildTarget::Wasm {} => sync
                    .write_wasm(&profile.name, ir.print_wasm(&exposed_mod_items))
                    .with_context(|| format!("sync: wasm profile '{}'", profile.name))?,
            }
        }

        if self.options.js.typescript {
            sync.write_ts(&ir).context("sync: ts")?;
        }

        for profile in &profiles {
            if matches!(profile.rust.target, ProfileBuildTarget::Wasm {}) {
                let abs_out_dir = sync.abs_out_dir();
                build_wasm(BuildWasmOpts {
                    project_root_abs_path: &project_root_abs_path,
                    tools_dir_abs_path: &abs_out_dir.join("tools"),
                    wasm_output_dir_abs_path: &abs_out_dir.join("output").join(&profile.name),
                    profile_name: Some(&profile.name),
                    release: false,
                })
                .with_context(|| format!("builder: wasm profile '{}'", profile.name))?;
            }
        }

        Ok(())
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
