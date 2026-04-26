//! Generate `default.rs` which contains mod declarations for all orphan modules
//! with `pub use` re-exports for treeshaked members only.

use std::path::{Path, PathBuf};

use crate::codegen::common::{mangle_module_name, needs_non_snake_case_allow, HEADER};
use crate::orphans::OrphanModuleShaked;

#[derive(Clone, Debug)]
pub struct GeneratedFile {
    /// Relative path (e.g. `lib.rs`, `modules/math.rs`). The emit layer
    /// joins this with the output directory.
    pub rel_path: PathBuf,
    pub content: String,
}

/// Generate `default.rs` with private mod declarations for all orphan modules
/// and `pub use` re-exports for only the JS-imported items.
pub fn generate_default_rs(
    modules: &[OrphanModuleShaked],
    path_prefix: &Path,
    crate_: &rustdoc_types::Crate,
    module_dir_rel_path: &Path,
    default_rel_path: PathBuf,
) -> GeneratedFile {
    let mut content = String::from(HEADER);

    for module in modules {
        let target_rel_path = path_prefix.join(&module.rel_rs_path);
        let target_rel_path = target_rel_path.to_string_lossy();

        let mod_name = mangle_module_name(&module.gen_module_name());

        content.push_str(&format!("#[path = \"{}\"]\n", target_rel_path));
        if needs_non_snake_case_allow(&mod_name) {
            content.push_str("#[allow(non_snake_case)]\n");
        }
        content.push_str(&format!("mod {};\n\n", mod_name));
    }

    for module in modules {
        let mod_name = mangle_module_name(&module.gen_module_name());

        for resolved in module.find_matching_members(crate_, module_dir_rel_path) {
            let Ok(id) = resolved else { continue };
            if let Some(item) = crate_.index.get(&id) {
                if let Some(name) = item.name.as_deref() {
                    content.push_str(&format!("pub use {mod_name}::{name};\n"));
                }
            }
        }
    }

    GeneratedFile {
        rel_path: default_rel_path,
        content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orphans::OrphanModuleMembers;
    use rustdoc_types::{Crate, Id, Item, ItemEnum, Span, Visibility};
    use std::collections::HashMap;

    fn make_item(id: u32, name: &str, filename: &str, visibility: Visibility) -> (Id, Item) {
        (
            Id(id),
            Item {
                id: Id(id),
                crate_id: 0,
                name: Some(name.to_string()),
                span: Some(Span {
                    filename: PathBuf::from(filename),
                    begin: (1, 0),
                    end: (1, 0),
                }),
                visibility,
                docs: None,
                links: HashMap::new(),
                attrs: Vec::new(),
                deprecation: None,
                inner: ItemEnum::Function(rustdoc_types::Function {
                    sig: rustdoc_types::FunctionSignature {
                        inputs: Vec::new(),
                        output: None,
                        is_c_variadic: false,
                    },
                    generics: rustdoc_types::Generics {
                        params: Vec::new(),
                        where_predicates: Vec::new(),
                    },
                    header: rustdoc_types::FunctionHeader {
                        is_const: false,
                        is_unsafe: false,
                        is_async: false,
                        abi: rustdoc_types::Abi::Rust,
                    },
                    has_body: true,
                }),
            },
        )
    }

    fn make_crate(items: Vec<(Id, Item)>) -> Crate {
        let index = items.into_iter().collect();
        Crate {
            root: Id(0),
            crate_version: None,
            includes_private: false,
            index,
            paths: HashMap::new(),
            external_crates: HashMap::new(),
            format_version: 0,
            target: rustdoc_types::Target {
                triple: String::new(),
                target_features: Vec::new(),
            },
        }
    }

    #[test]
    fn generates_mod_declarations_for_all_modules() {
        let modules = vec![
            OrphanModuleShaked {
                rel_rs_path: PathBuf::from("greet.rs"),
                imported_members: OrphanModuleMembers::Specific(vec!["hello".into()]),
            },
            OrphanModuleShaked {
                rel_rs_path: PathBuf::from("unused.rs"),
                imported_members: OrphanModuleMembers::None,
            },
        ];

        let crate_ = make_crate(vec![
            make_item(1, "hello", "src/greet.rs", Visibility::Public),
            make_item(2, "unused_fn", "src/unused.rs", Visibility::Public),
        ]);

        let file = generate_default_rs(
            &modules,
            Path::new("../src"),
            &crate_,
            Path::new("src"),
            PathBuf::from("default.rs"),
        );

        assert_eq!(file.rel_path, PathBuf::from("default.rs"));

        let body = &file.content[HEADER.len()..];
        assert_eq!(
            body,
            concat!(
                "#[path = \"../src/greet.rs\"]\n",
                "mod greet;\n",
                "\n",
                "#[path = \"../src/unused.rs\"]\n",
                "mod unused;\n",
                "\n",
                "pub use greet::hello;\n",
            )
        );
    }

    #[test]
    fn mangled_names_get_allow_attribute() {
        let modules = vec![OrphanModuleShaked {
            rel_rs_path: PathBuf::from("nested/deep.rs"),
            imported_members: OrphanModuleMembers::Specific(vec!["run".into()]),
        }];

        let crate_ = make_crate(vec![make_item(
            1,
            "run",
            "src/nested/deep.rs",
            Visibility::Public,
        )]);

        let file = generate_default_rs(
            &modules,
            Path::new("../../src"),
            &crate_,
            Path::new("src"),
            PathBuf::from("default.rs"),
        );

        let body = &file.content[HEADER.len()..];
        assert_eq!(
            body,
            concat!(
                "#[path = \"../../src/nested/deep.rs\"]\n",
                "#[allow(non_snake_case)]\n",
                "mod nested__deep;\n",
                "\n",
                "pub use nested__deep::run;\n",
            )
        );
    }

    #[test]
    fn no_reexports_when_nothing_imported() {
        let modules = vec![OrphanModuleShaked {
            rel_rs_path: PathBuf::from("greet.rs"),
            imported_members: OrphanModuleMembers::None,
        }];

        let crate_ = make_crate(vec![make_item(
            1,
            "hello",
            "src/greet.rs",
            Visibility::Public,
        )]);

        let file = generate_default_rs(
            &modules,
            Path::new("../src"),
            &crate_,
            Path::new("src"),
            PathBuf::from("default.rs"),
        );

        let body = &file.content[HEADER.len()..];
        assert_eq!(
            body,
            concat!("#[path = \"../src/greet.rs\"]\n", "mod greet;\n", "\n",)
        );
    }

    #[test]
    fn star_import_re_exports_public_only() {
        let modules = vec![OrphanModuleShaked {
            rel_rs_path: PathBuf::from("math.rs"),
            imported_members: OrphanModuleMembers::All,
        }];

        let crate_ = make_crate(vec![
            make_item(1, "add", "src/math.rs", Visibility::Public),
            make_item(2, "sub", "src/math.rs", Visibility::Public),
            make_item(3, "internal", "src/math.rs", Visibility::Default),
        ]);

        let file = generate_default_rs(
            &modules,
            Path::new("../src"),
            &crate_,
            Path::new("src"),
            PathBuf::from("default.rs"),
        );

        let body = &file.content[HEADER.len()..];
        assert!(body.contains("pub use math::add;\n"));
        assert!(body.contains("pub use math::sub;\n"));
        assert!(!body.contains("internal"));
    }
}
