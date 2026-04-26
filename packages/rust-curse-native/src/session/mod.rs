use std::path::PathBuf;

use anyhow::Context;
#[cfg(feature = "napi-binding")]
use napi_derive::napi;

use crate::orphans::{self, OrphanScanOpts, SourceFilter};
use crate::scaffold::rustdoc::RustdocSectionOpts;
use crate::scaffold::{Scaffold, ScaffoldOptions};

#[cfg_attr(feature = "napi-binding", napi)]
pub enum BuildTarget {
    Default {
        js_source: SourceOptions,
    },
    Node {
        js_source: SourceOptions,
        feature_name: String,
    },
    Wasm {
        js_source: SourceOptions,
        feature_name: String,
    },
}

impl BuildTarget {
    /// Cargo feature name that activates this target, if any.
    pub fn feature_name(&self) -> Option<&str> {
        match self {
            BuildTarget::Default { .. } => None,
            BuildTarget::Node { feature_name, .. } => Some(feature_name),
            BuildTarget::Wasm { feature_name, .. } => Some(feature_name),
        }
    }

    /// File name for the generated module (e.g. `"node.rs"`).
    pub fn file_name(&self) -> &'static str {
        match self {
            BuildTarget::Default { .. } => "default.rs",
            BuildTarget::Node { .. } => "node.rs",
            BuildTarget::Wasm { .. } => "wasm.rs",
        }
    }

    pub fn js_source(&self) -> &SourceOptions {
        match self {
            BuildTarget::Default { js_source, .. }
            | BuildTarget::Node { js_source, .. }
            | BuildTarget::Wasm { js_source, .. } => js_source,
        }
    }
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct SourceOptions {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct SessionOptions {
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
pub struct Session {
    options: SessionOptions,
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
impl Session {
    #[cfg_attr(feature = "napi-binding", napi(constructor))]
    pub fn new(options: SessionOptions) -> Self {
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
