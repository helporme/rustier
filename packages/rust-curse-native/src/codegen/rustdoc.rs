//! Generate `rustdoc.rs` that declares orphan modules with `pub mod`
//! so rustdoc can index all public items.

use crate::codegen::common::{mangle_module_name, needs_non_snake_case_allow, HEADER};
use crate::model::{GeneratedFile, OrphanModule};
use std::path::{Path, PathBuf};

/// Generate `rustdoc.rs` that declares all orphan modules as `pub mod`.
///
/// `path_prefix` is prepended to each module's `rel_path` in the `#[path]` attribute.
pub fn generate_rustdoc_rs(
    modules: &[OrphanModule],
    path_prefix: &Path,
    rustdoc_rel_path: PathBuf,
) -> GeneratedFile {
    let mut content = String::from(HEADER);

    for module in modules {
        let target_rel_path = path_prefix.join(&module.rel_path);
        let target_rel_path = target_rel_path.to_string_lossy();

        let mod_name = mangle_module_name(&module.name);

        content.push_str(&format!("#[path = \"{}\"]\n", target_rel_path));
        if needs_non_snake_case_allow(&mod_name) {
            content.push_str("#[allow(non_snake_case)]\n");
        }
        content.push_str(&format!("pub mod {};\n\n", mod_name));
    }

    GeneratedFile {
        rel_path: rustdoc_rel_path,
        content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_simple_modules() {
        let modules = vec![
            OrphanModule {
                name: "greet".into(),
                rel_path: PathBuf::from("greet.rs"),
                item_ids: Vec::new(),
            },
            OrphanModule {
                name: "math".into(),
                rel_path: PathBuf::from("math.rs"),
                item_ids: Vec::new(),
            },
        ];

        let file = generate_rustdoc_rs(
            &modules,
            Path::new("../../../src"),
            PathBuf::from("rustdoc.rs"),
        );

        assert_eq!(file.rel_path, PathBuf::from("rustdoc.rs"));
        assert!(file.content.starts_with(HEADER));
        let body = &file.content[HEADER.len()..];
        assert_eq!(
            body,
            concat!(
                "#[path = \"../../../src/greet.rs\"]\n",
                "pub mod greet;\n",
                "\n",
                "#[path = \"../../../src/math.rs\"]\n",
                "pub mod math;\n",
                "\n",
            )
        );
    }

    #[test]
    fn mangles_nested_module_names() {
        let modules = vec![OrphanModule {
            name: "nested::deep".into(),
            rel_path: PathBuf::from("nested/deep.rs"),
            item_ids: Vec::new(),
        }];

        let file = generate_rustdoc_rs(
            &modules,
            Path::new("../../src"),
            PathBuf::from("rustdoc.rs"),
        );

        let body = &file.content[HEADER.len()..];
        assert_eq!(
            body,
            concat!(
                "#[path = \"../../src/nested/deep.rs\"]\n",
                "#[allow(non_snake_case)]\n",
                "pub mod nested__deep;\n",
                "\n",
            )
        );
    }

    #[test]
    fn underscored_names_dont_collide() {
        let modules = vec![
            OrphanModule {
                name: "nested::file".into(),
                rel_path: PathBuf::from("nested/file.rs"),
                item_ids: Vec::new(),
            },
            OrphanModule {
                name: "nested__::file".into(),
                rel_path: PathBuf::from("nested__/file.rs"),
                item_ids: Vec::new(),
            },
        ];

        let file = generate_rustdoc_rs(&modules, Path::new("../src"), PathBuf::from("rustdoc.rs"));

        let body = &file.content[HEADER.len()..];
        assert_eq!(
            body,
            concat!(
                "#[path = \"../src/nested/file.rs\"]\n",
                "#[allow(non_snake_case)]\n",
                "pub mod nested__file;\n",
                "\n",
                "#[path = \"../src/nested__/file.rs\"]\n",
                "#[allow(non_snake_case)]\n",
                "pub mod nested0_0___file;\n",
                "\n",
            )
        );
    }

    #[test]
    fn underscored_directory_names() {
        let modules = vec![OrphanModule {
            name: "__nested::__deep".into(),
            rel_path: PathBuf::from("__nested/__deep.rs"),
            item_ids: Vec::new(),
        }];

        let file = generate_rustdoc_rs(&modules, Path::new("../src"), PathBuf::from("rustdoc.rs"));

        let body = &file.content[HEADER.len()..];
        assert_eq!(
            body,
            concat!(
                "#[path = \"../src/__nested/__deep.rs\"]\n",
                "#[allow(non_snake_case)]\n",
                "pub mod 0_0_nested__0_0_deep;\n",
                "\n",
            )
        );
    }

}
