//! Default fallback section: active when no cargo feature and no env
//! flag are set, primarily so rust-analyzer has something to load.

use std::collections::HashSet;

use crate::common::{mangle_module_name, needs_non_snake_case_allow, HEADER};
use crate::ir::{Id, Item, IR};
use crate::orphans::OrphanModule;

use super::{Scaffold, ScaffoldError, ScaffoldSection};

pub struct DefaultSectionOpts<'a> {
    pub modules: &'a [OrphanModule],
    pub ir: &'a IR,
    pub exposed: &'a HashSet<Id>,
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
            let mod_name = mangle_module_name(&module.mod_name);

            content.push_str(&format!("#[path = \"{}\"]\n", target_rel_path));
            if needs_non_snake_case_allow(&mod_name) {
                content.push_str("#[allow(non_snake_case)]\n");
            }
            content.push_str(&format!("mod {};\n\n", mod_name));
        }

        for module in opts.modules {
            let mangled = mangle_module_name(&module.mod_name);
            let Some(ir_mod) = find_module(opts.ir, &mangled) else {
                continue;
            };

            let mut names: Vec<&str> = opts
                .ir
                .get_mod_unchecked(ir_mod)
                .exports
                .iter()
                .filter(|e| opts.exposed.contains(&e.target))
                .map(|e| e.name.as_str())
                .collect();
            names.sort_unstable();

            for name in names {
                content.push_str(&format!("pub use {mangled}::{name};\n"));
            }
        }

        content
    }
}

fn find_module(ir: &IR, mangled_name: &str) -> Option<Id> {
    let expected: Vec<&str> = vec![ir.crate_name.as_str(), mangled_name];
    for (i, item) in ir.items.iter().enumerate() {
        let Item::Mod(def) = item else { continue };
        let segments: Vec<&str> = def.path.segments.iter().map(String::as_str).collect();
        if segments == expected {
            return Some(Id(i as u32));
        }
    }
    None
}
