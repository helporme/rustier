//! Default fallback section: active when no cargo feature and no env
//! flag are set, primarily so rust-analyzer has something to load.

use std::path::Path;

use crate::common::{mangle_module_name, needs_non_snake_case_allow, HEADER};
use crate::orphans::OrphanModuleShaked;

use super::{Scaffold, ScaffoldError, ScaffoldSection};

pub struct DefaultSectionOpts<'a> {
    pub modules: &'a [OrphanModuleShaked],
    pub crate_: &'a rustdoc_types::Crate,
    pub module_dir_rel_path: &'a Path,
}

impl Scaffold {
    pub fn set_default(&mut self, opts: DefaultSectionOpts) -> Result<(), ScaffoldError> {
        let section = ScaffoldSection::new("default")
            .cargo_feature_default()
            .lib(self.generate_default_lib_content(&opts));

        self.set(section)
    }

    fn generate_default_lib_content(&self, opts: &DefaultSectionOpts) -> String {
        let mut content = String::from(HEADER);

        for module in opts.modules {
            let target_rel_path = self.abs_path_from_project_root_to(&module.rel_rs_path);
            let target_rel_path = target_rel_path.to_string_lossy();
            let mod_name = mangle_module_name(&module.gen_module_name());

            content.push_str(&format!("#[path = \"{}\"]\n", target_rel_path));
            if needs_non_snake_case_allow(&mod_name) {
                content.push_str("#[allow(non_snake_case)]\n");
            }
            content.push_str(&format!("mod {};\n\n", mod_name));
        }

        for module in opts.modules {
            let mod_name = mangle_module_name(&module.gen_module_name());

            let mut names: Vec<&str> = module
                .find_matching_members(opts.crate_, opts.module_dir_rel_path)
                .filter_map(Result::ok)
                .filter_map(|id| opts.crate_.index.get(&id))
                .filter_map(|item| item.name.as_deref())
                .collect();
            names.sort_unstable();

            for name in names {
                content.push_str(&format!("pub use {mod_name}::{name};\n"));
            }
        }

        content
    }
}
