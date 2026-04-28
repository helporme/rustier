pub mod common; // todo: should be private
pub mod ir;
pub mod orphans;
pub mod rustdoc;
pub mod scaffold;

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Context;
#[cfg(feature = "napi-binding")]
use napi_derive::napi;
use rustdoc_types::{Crate, Id};

use crate::ir::from_rustdoc::build_ir;
use crate::ir::IR;
use crate::orphans::{OrphanModuleShaked, OrphanScanOpts, OrphanShakeOpts, SourceFilter};
use crate::scaffold::default::DefaultSectionOpts;
use crate::scaffold::napi::NapiSectionOpts;
use crate::scaffold::rustdoc::RustdocSectionOpts;
use crate::scaffold::wasm::WasmSectionOpts;
use crate::scaffold::{Scaffold, ScaffoldError, ScaffoldOptions};

#[cfg_attr(feature = "napi-binding", napi)]
pub enum BuildTarget {
    Default { js_source: SourceOptions },
    Node { js_source: SourceOptions },
    Wasm { js_source: SourceOptions },
}

impl BuildTarget {
    fn js_source(&self) -> &SourceOptions {
        match self {
            BuildTarget::Default { js_source }
            | BuildTarget::Node { js_source }
            | BuildTarget::Wasm { js_source } => js_source,
        }
    }

    fn section_name(&self) -> &'static str {
        match self {
            BuildTarget::Default { .. } => "default",
            BuildTarget::Node { .. } => "napi",
            BuildTarget::Wasm { .. } => "wasm",
        }
    }

    fn set_scaffold_section(
        &self,
        scaffold: &mut Scaffold,
        shaked: &[OrphanModuleShaked],
        rustdoc: &Crate,
        ir: &IR,
    ) -> Result<(), ScaffoldError> {
        match self {
            BuildTarget::Default { .. } => scaffold.set_default(DefaultSectionOpts {
                modules: shaked,
                crate_: rustdoc,
            }),
            BuildTarget::Node { .. } => scaffold.set_napi(NapiSectionOpts {
                feature_name: "napi".into(),
                lib_content: ir.print_napi(),
            }),
            BuildTarget::Wasm { .. } => scaffold.set_wasm(WasmSectionOpts {
                feature_name: "wasm".into(),
                lib_content: ir.print_wasm(),
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
        let project_root = PathBuf::from(&self.options.project_root_abs_path);
        let mod_root_rel_paths = vec![PathBuf::from(&self.options.module_dir_rel_path)];

        let modules = orphans::scan(OrphanScanOpts {
            project_root_abs_path: &project_root,
            mod_root_rel_paths: &mod_root_rel_paths,
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

        for target in &self.options.targets {
            let js_source = target.js_source();

            let shaked = orphans::shake(
                &modules,
                OrphanShakeOpts {
                    project_root_abs_path: &project_root,
                    mod_root_rel_paths: &mod_root_rel_paths,
                    js_filter: SourceFilter {
                        include: &js_source.include,
                        exclude: &js_source.exclude,
                    },
                },
            )?;

            let allowed: HashSet<Id> = shaked
                .iter()
                .flat_map(|m| m.find_matching_members(&rustdoc))
                .filter_map(Result::ok) // todo: do not silence errors
                .collect();

            let ir = build_ir(&allowed, &rustdoc).context("build ir")?;

            target
                .set_scaffold_section(&mut scaffold, &shaked, &rustdoc, &ir)
                .with_context(|| format!("scaffold: {}", target.section_name()))?;
        }

        scaffold.deploy().context("scaffold: deploy targets")?;

        Ok(BuildResult::empty())
    }
}
