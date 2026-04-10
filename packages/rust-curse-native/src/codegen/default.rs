//! Generate `default.rs` which contains mod declarations for all orphan modules
//! with `pub use` re-exports for treeshaked members only.

use std::path::{Path, PathBuf};

use crate::codegen::common::{mangle_module_name, needs_non_snake_case_allow, HEADER};
use crate::model::{GeneratedFile, OrphanModule, ShakedModule};

/// Generate `default.rs` with private mod declarations for all orphan modules
/// and `pub use` re-exports for only the treeshaked items.
pub fn generate_default_rs(
    modules: &[OrphanModule],
    shaked: &[ShakedModule],
    path_prefix: &Path,
    crate_: &rustdoc_types::Crate,
    default_rel_path: PathBuf,
) -> GeneratedFile {
    let mut content = String::from(HEADER);

    // Emit mod declarations for all orphan modules
    for module in modules {
        let target_rel_path = path_prefix.join(&module.rel_path);
        let target_rel_path = target_rel_path.to_string_lossy();

        let mod_name = mangle_module_name(&module.name);

        content.push_str(&format!("#[path = \"{}\"]\n", target_rel_path));
        if needs_non_snake_case_allow(&mod_name) {
            content.push_str("#[allow(non_snake_case)]\n");
        }
        content.push_str(&format!("mod {};\n\n", mod_name));
    }

    // Emit pub use re-exports for treeshaked items
    for shaked_module in shaked {
        let mod_name = mangle_module_name(&shaked_module.module.name);

        for id in &shaked_module.imported_item_ids {
            if let Some(item) = crate_.index.get(id) {
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
    use rustdoc_types::{Crate, Id, Item, ItemEnum, Visibility};
    use std::collections::HashMap;

    fn make_item(id: u32, name: &str, visibility: Visibility) -> (Id, Item) {
        (
            Id(id),
            Item {
                id: Id(id),
                crate_id: 0,
                name: Some(name.to_string()),
                span: None,
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
            OrphanModule {
                name: "greet".into(),
                rel_path: PathBuf::from("greet.rs"),
                item_ids: vec![Id(1)],
            },
            OrphanModule {
                name: "unused".into(),
                rel_path: PathBuf::from("unused.rs"),
                item_ids: vec![Id(2)],
            },
        ];

        let shaked = vec![ShakedModule {
            module: modules[0].clone(),
            importer_abs_paths: vec![PathBuf::from("/app/main.js")],
            imported_item_ids: vec![Id(1)],
        }];

        let crate_ = make_crate(vec![
            make_item(1, "hello", Visibility::Public),
            make_item(2, "unused_fn", Visibility::Public),
        ]);

        let file = generate_default_rs(
            &modules,
            &shaked,
            Path::new("../src"),
            &crate_,
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
        let modules = vec![OrphanModule {
            name: "nested::deep".into(),
            rel_path: PathBuf::from("nested/deep.rs"),
            item_ids: vec![Id(1)],
        }];

        let shaked = vec![ShakedModule {
            module: modules[0].clone(),
            importer_abs_paths: vec![PathBuf::from("/app/main.js")],
            imported_item_ids: vec![Id(1)],
        }];

        let crate_ = make_crate(vec![make_item(1, "run", Visibility::Public)]);

        let file = generate_default_rs(
            &modules,
            &shaked,
            Path::new("../../src"),
            &crate_,
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
    fn no_reexports_when_nothing_shaked() {
        let modules = vec![OrphanModule {
            name: "greet".into(),
            rel_path: PathBuf::from("greet.rs"),
            item_ids: vec![Id(1)],
        }];

        let crate_ = make_crate(vec![make_item(1, "hello", Visibility::Public)]);

        let file = generate_default_rs(
            &modules,
            &[], // no shaked modules
            Path::new("../src"),
            &crate_,
            PathBuf::from("default.rs"),
        );

        let body = &file.content[HEADER.len()..];
        assert_eq!(
            body,
            concat!("#[path = \"../src/greet.rs\"]\n", "mod greet;\n", "\n",)
        );
    }

    #[test]
    fn multiple_items_reexported() {
        let modules = vec![OrphanModule {
            name: "math".into(),
            rel_path: PathBuf::from("math.rs"),
            item_ids: vec![Id(1), Id(2), Id(3)],
        }];

        let shaked = vec![ShakedModule {
            module: modules[0].clone(),
            importer_abs_paths: vec![PathBuf::from("/app/main.js")],
            imported_item_ids: vec![Id(1), Id(2)],
        }];

        let crate_ = make_crate(vec![
            make_item(1, "add", Visibility::Public),
            make_item(2, "sub", Visibility::Public),
            make_item(3, "internal", Visibility::Default),
        ]);

        let file = generate_default_rs(
            &modules,
            &shaked,
            Path::new("../src"),
            &crate_,
            PathBuf::from("default.rs"),
        );

        let body = &file.content[HEADER.len()..];
        assert!(body.contains("pub use math::add;\n"));
        assert!(body.contains("pub use math::sub;\n"));
        assert!(!body.contains("internal"));
    }
}
