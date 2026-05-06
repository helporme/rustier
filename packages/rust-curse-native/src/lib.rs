pub mod builder;
pub mod common; // todo: should be private
pub mod ir;
pub mod orphans;
pub mod rustdoc;
pub mod scaffold;
pub mod sync;
pub mod tree_shake;

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Context;
#[cfg(feature = "napi-binding")]
use napi_derive::napi;

use crate::builder::wasm::{build_wasm, BuildWasmOpts};
use crate::ir::{Id, IR};
use crate::orphans::{OrphanScanOpts, SourceFilter};
use crate::scaffold::link::LinkSectionOpts;
use crate::scaffold::napi::NapiSectionOpts;
use crate::scaffold::rustdoc::RustdocSectionOpts;
use crate::scaffold::wasm::WasmSectionOpts;
use crate::scaffold::{Scaffold, ScaffoldError, ScaffoldOptions};
use crate::tree_shake::TreeShakeOpts;

#[cfg_attr(feature = "napi-binding", napi)]
pub enum BuildTarget {
    Node,
    Wasm,
    Ts,
}

impl BuildTarget {
    fn section_name(&self) -> &'static str {
        match self {
            BuildTarget::Node => "napi",
            BuildTarget::Wasm => "wasm",
            BuildTarget::Ts => "ts",
        }
    }

    fn set_scaffold_section(
        &self,
        scaffold: &mut Scaffold,
        ir: &IR,
        exposed: &HashSet<Id>,
    ) -> Result<(), ScaffoldError> {
        match self {
            BuildTarget::Node => scaffold.set_napi(NapiSectionOpts {
                content: ir.print_napi(exposed),
            }),
            BuildTarget::Wasm => scaffold.set_wasm(WasmSectionOpts {
                content: ir.print_wasm(exposed),
            }),
            BuildTarget::Ts => scaffold.set_ts(ir),
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
    /// Glob filter applied to JS sources to determine which Rust items
    /// are exposed to the bindings. Shared by every target.
    pub js_source: SourceOptions,
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
        let project_root_abs_path = PathBuf::from(&self.options.project_root_abs_path);
        let src_rel_path = PathBuf::from(&self.options.module_dir_rel_path);

        let modules = orphans::scan(OrphanScanOpts {
            project_root_abs_path: &project_root_abs_path,
            src_rel_path: &src_rel_path,
            rs_filter: SourceFilter::default(), // todo: add as an option
        })?;

        let mut scaffold = Scaffold::new(ScaffoldOptions {
            abs_root_path: project_root_abs_path.clone(),
            temp_folder_rel_path: PathBuf::from(&self.options.output_dir_rel_path),
            cargo_toml_rel_path: PathBuf::from(&self.options.cargo_toml_rel_path),
        })
        .context("scaffold")?;

        scaffold
            .set_rustdoc(RustdocSectionOpts { modules: &modules })
            .context("scaffold: rustdoc")?;

        scaffold.deploy().context("scaffold: deploy rustdoc")?;

        let rustdoc = rustdoc::gen(&project_root_abs_path)?;
        let ir = IR::try_from(&rustdoc).context("build ir")?;

        let exposed = tree_shake::shake(
            &ir,
            TreeShakeOpts {
                project_root_abs_path: &project_root_abs_path,
                src_rel_path: &src_rel_path,
                js_filter: SourceFilter {
                    include: &self.options.js_source.include,
                    exclude: &self.options.js_source.exclude,
                },
            },
        )?;

        scaffold
            .set_link(LinkSectionOpts {
                modules: &modules,
                ir: &ir,
                exposed: &exposed,
            })
            .context("scaffold: link")?;

        for target in &self.options.targets {
            target
                .set_scaffold_section(&mut scaffold, &ir, &exposed)
                .with_context(|| format!("scaffold: {}", target.section_name()))?;
        }

        scaffold.deploy().context("scaffold: deploy targets")?;

        for target in &self.options.targets {
            if matches!(target, BuildTarget::Wasm) {
                build_wasm(BuildWasmOpts {
                    project_root_abs_path: &project_root_abs_path,
                    tools_dir_abs_path: &scaffold.abs_path_to("tools"),
                    wasm_output_dir_abs_path: &scaffold.abs_path_to("output/wasm"),
                    release: false,
                })
                .context("builder: wasm")?;
            }
        }

        Ok(())
    }
}
