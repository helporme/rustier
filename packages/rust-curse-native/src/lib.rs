pub mod common; // todo: should be private
pub mod ir;
pub mod orphans;
pub mod rustdoc;
pub mod scaffold;
pub mod tree_shake;

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Context;
#[cfg(feature = "napi-binding")]
use napi_derive::napi;

use crate::ir::{Id, IR};
use crate::orphans::{OrphanModule, OrphanScanOpts, SourceFilter};
use crate::scaffold::default::DefaultSectionOpts;
use crate::scaffold::napi::NapiSectionOpts;
use crate::scaffold::rustdoc::RustdocSectionOpts;
use crate::scaffold::ts::TsSectionOpts;
use crate::scaffold::wasm::WasmSectionOpts;
use crate::scaffold::{Scaffold, ScaffoldError, ScaffoldOptions};
use crate::tree_shake::TreeShakeOpts;

#[cfg_attr(feature = "napi-binding", napi)]
pub enum BuildTarget {
    Default { js_source: SourceOptions },
    Node { js_source: SourceOptions },
    Wasm { js_source: SourceOptions },
    Ts { js_source: SourceOptions },
}

impl BuildTarget {
    fn js_source(&self) -> &SourceOptions {
        match self {
            BuildTarget::Default { js_source }
            | BuildTarget::Node { js_source }
            | BuildTarget::Wasm { js_source }
            | BuildTarget::Ts { js_source } => js_source,
        }
    }

    fn section_name(&self) -> &'static str {
        match self {
            BuildTarget::Default { .. } => "default",
            BuildTarget::Node { .. } => "napi",
            BuildTarget::Wasm { .. } => "wasm",
            BuildTarget::Ts { .. } => "ts",
        }
    }

    fn set_scaffold_section(
        &self,
        scaffold: &mut Scaffold,
        modules: &[OrphanModule],
        ir: &IR,
        exposed: &HashSet<Id>,
    ) -> Result<(), ScaffoldError> {
        match self {
            BuildTarget::Default { .. } => scaffold.set_default(DefaultSectionOpts {
                modules,
                ir,
                exposed,
            }),
            BuildTarget::Node { .. } => scaffold.set_napi(NapiSectionOpts {
                feature_name: "napi".into(),
                content: ir.print_napi(exposed),
            }),
            BuildTarget::Wasm { .. } => scaffold.set_wasm(WasmSectionOpts {
                feature_name: "wasm".into(),
                content: ir.print_wasm(exposed),
            }),
            BuildTarget::Ts { .. } => scaffold.set_ts(TsSectionOpts {
                content: ir.print_ts(),
            }),
        }
    }
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

#[cfg_attr(feature = "napi-binding", napi)]
impl RustCurse {
    #[cfg_attr(feature = "napi-binding", napi(constructor))]
    pub fn new(options: RustCurseOptions) -> Self {
        Self { options }
    }

    #[cfg(feature = "napi-binding")]
    #[napi]
    pub fn build(&mut self) -> napi::Result<(), &'static str> {
        self.try_build()
            .map_err(|e| napi::Error::new("rust-curse error", format!("{:#}", e)))
    }

    fn try_build(&mut self) -> anyhow::Result<()> {
        let project_root = PathBuf::from(&self.options.project_root_abs_path);
        let src_rel_path = PathBuf::from(&self.options.module_dir_rel_path);

        let modules = orphans::scan(OrphanScanOpts {
            project_root_abs_path: &project_root,
            src_rel_path: &src_rel_path,
            rs_filter: SourceFilter::default(), // todo: add as an option
        })?;

        let mut scaffold = Scaffold::new(ScaffoldOptions {
            abs_root_path: project_root.clone(),
            temp_folder_rel_path: PathBuf::from(&self.options.output_dir_rel_path),
            cargo_toml_rel_path: PathBuf::from(&self.options.cargo_toml_rel_path),
        })
        .context("scaffold")?;

        scaffold
            .set_rustdoc(RustdocSectionOpts { modules: &modules })
            .context("scaffold: rustdoc")?;

        scaffold.deploy().context("scaffold: deploy rustdoc")?;

        let rustdoc = rustdoc::gen(&project_root)?;
        let ir = IR::try_from(&rustdoc).context("build ir")?;

        for target in &self.options.targets {
            let js_source = target.js_source();

            let exposed = tree_shake::shake(
                &ir,
                TreeShakeOpts {
                    project_root_abs_path: &project_root,
                    src_rel_path: &src_rel_path,
                    js_filter: SourceFilter {
                        include: &js_source.include,
                        exclude: &js_source.exclude,
                    },
                },
            )?;

            target
                .set_scaffold_section(&mut scaffold, &modules, &ir, &exposed)
                .with_context(|| format!("scaffold: {}", target.section_name()))?;
        }

        scaffold.deploy().context("scaffold: deploy targets")?;

        Ok(())
    }
}
