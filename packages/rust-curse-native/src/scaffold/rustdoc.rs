use crate::{
    common::{mangle_module_name, needs_non_snake_case_allow, HEADER},
    orphans::OrphanModule,
};

use super::{Scaffold, ScaffoldError, ScaffoldSection};

pub const RUSTDOC_ENV_VAR: &str = "RUST_CURSE_RUSTDOC";

pub struct RustdocSectionOpts<'a> {
    pub modules: &'a [OrphanModule],
}

impl Scaffold {
    pub fn set_rustdoc(&mut self, opts: RustdocSectionOpts) -> Result<(), ScaffoldError> {
        let section = ScaffoldSection::new("rustdoc")
            .env_flag(RUSTDOC_ENV_VAR)
            .lib(self.generate_lib_content(opts.modules));

        self.set(section)
    }

    fn generate_lib_content(&self, modules: &[OrphanModule]) -> String {
        let mut content = String::from(HEADER);

        for module in modules {
            let target_rel_path = self.abs_path_from_project_root_to(&module.rel_rs_path);
            let target_rel_path = target_rel_path.to_string_lossy();

            let mod_name = mangle_module_name(&module.mod_name);

            content.push_str(&format!("#[path = \"{}\"]\n", target_rel_path));
            if needs_non_snake_case_allow(&mod_name) {
                content.push_str("#[allow(non_snake_case)]\n");
            }
            content.push_str(&format!("pub mod {};\n\n", mod_name));
        }

        content
    }
}
