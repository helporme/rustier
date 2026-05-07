pub mod builder;
pub mod common; // todo: should be private
pub mod ir;
pub mod orphans;
pub mod rustdoc;
pub mod sync;
pub mod tree_shake;

use std::path::PathBuf;

use anyhow::Context;
#[cfg(feature = "napi-binding")]
use napi_derive::napi;

use crate::builder::wasm::{build_wasm, BuildWasmOpts};
use crate::ir::IR;
use crate::orphans::{OrphanScanOpts, SourceFilter};
use crate::sync::write_orphan_tree::WriteOrphanTreeOpts;
use crate::sync::Syncer;
use crate::tree_shake::TreeShakeOpts;

#[cfg_attr(feature = "napi-binding", napi)]
pub enum BuildTarget {
    Node,
    Wasm,
    Ts,
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

        let mut syncer = Syncer::new(
            project_root_abs_path.clone(),
            PathBuf::from(&self.options.output_dir_rel_path),
        );

        syncer
            .write_wired_cargo_toml()
            .context("sync: wired Cargo.toml")?;

        syncer.write_rustdoc(&modules).context("sync: rustdoc")?;

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

        syncer
            .write_orphan_tree(WriteOrphanTreeOpts {
                modules: &modules,
                ir: &ir,
                exposed: &exposed,
            })
            .context("sync: orphan_tree")?;

        for target in &self.options.targets {
            match target {
                BuildTarget::Node => syncer
                    .write_napi(ir.print_napi(&exposed))
                    .context("sync: napi")?,
                BuildTarget::Wasm => syncer
                    .write_wasm(ir.print_wasm(&exposed))
                    .context("sync: wasm")?,
                BuildTarget::Ts => syncer.write_ts(&ir).context("sync: ts")?,
            }
        }

        for target in &self.options.targets {
            if matches!(target, BuildTarget::Wasm) {
                let abs_out_dir = syncer.abs_out_dir();
                build_wasm(BuildWasmOpts {
                    project_root_abs_path: &project_root_abs_path,
                    tools_dir_abs_path: &abs_out_dir.join("tools"),
                    wasm_output_dir_abs_path: &abs_out_dir.join("output/wasm"),
                    release: false,
                })
                .context("builder: wasm")?;
            }
        }

        Ok(())
    }
}
