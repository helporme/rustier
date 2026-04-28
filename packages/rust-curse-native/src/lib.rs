pub(crate) mod common;
pub(crate) mod ir;
pub(crate) mod orphans;
pub(crate) mod rustdoc;
pub(crate) mod scaffold;

use std::path::PathBuf;

use anyhow::Context;
#[cfg(feature = "napi-binding")]
use napi_derive::napi;

use crate::orphans::{OrphanScanOpts, SourceFilter};
use crate::scaffold::rustdoc::RustdocSectionOpts;
use crate::scaffold::{Scaffold, ScaffoldOptions};

#[cfg_attr(feature = "napi-binding", napi)]
pub enum BuildTarget {
    Default { js_source: SourceOptions },
    Node { js_source: SourceOptions },
    Wasm { js_source: SourceOptions },
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct SourceOptions {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct RustCurseOptions {
    pub targets: Vec<BuildTarget>,
    /// Directory to discover rust orphan modules, relative to project root.
    pub module_dir_rel_path: String,
    /// Output dir for placing generated files, relative to project root.
    pub output_dir_rel_path: String,
    /// Path to the original Cargo.toml, relative to project root.
    pub cargo_toml_rel_path: String,
    /// Absolute path to the project root directory.
    pub project_root_abs_path: String,
}

#[cfg_attr(feature = "napi-binding", napi)]
pub struct RustCurse {
    options: RustCurseOptions,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct BuildResult {
    pub warnings: Vec<String>,
}

impl BuildResult {
    fn empty() -> Self {
        Self {
            warnings: Vec::new(),
        }
    }
}

#[cfg_attr(feature = "napi-binding", napi)]
impl RustCurse {
    #[cfg_attr(feature = "napi-binding", napi(constructor))]
    pub fn new(options: RustCurseOptions) -> Self {
        Self { options }
    }

    #[cfg_attr(feature = "napi-binding", napi)]
    pub fn build(&mut self) -> BuildResult {
        self.try_build().expect("build failed")
    }

    fn try_build(&mut self) -> anyhow::Result<BuildResult> {
        let module_root = PathBuf::from(&self.options.project_root_abs_path)
            .join(&self.options.module_dir_rel_path);

        let modules = orphans::scan(OrphanScanOpts {
            root_abs_path: &module_root,
            rs_filter: SourceFilter::default(),
        })?;

        let mut scaffold = Scaffold::new(ScaffoldOptions {
            abs_root_path: PathBuf::from(&self.options.project_root_abs_path),
            temp_folder_rel_path: PathBuf::from(&self.options.output_dir_rel_path),
            cargo_toml_rel_path: PathBuf::from(&self.options.cargo_toml_rel_path),
        })
        .context("scaffold")?;

        scaffold
            .rustdoc(RustdocSectionOpts { modules: &modules })
            .context("scaffold: rustdoc")?;

        scaffold.deploy().context("scaffold: deploy rustdoc")?;

        // 1) gen rustdoc
        // 2) gen ir
        // 3) scaffold default/wasm/napi

        Ok(BuildResult::empty())
    }
}
