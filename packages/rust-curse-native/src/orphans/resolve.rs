//! Module-name derivation and rustdoc member resolution for [`OrphanModule`].

use std::collections::HashSet;
use std::path::Path;

use crate::rustdoc::normalize_path;

use super::{OrphanModuleMembers, OrphanModuleShaked};

/// Derive the `::`-separated rust module name from a relative `.rs` path.
///
/// `foo/bar.rs` -> `foo::bar`, `foo/mod.rs` -> `foo`.
pub fn gen_module_name(rel_rs_path: &Path) -> String {
    let without_ext = rel_rs_path.with_extension("");

    let segments: Vec<&str> = without_ext
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    let trimmed = if segments.last() == Some(&"mod") {
        &segments[..segments.len() - 1]
    } else {
        &segments
    };

    trimmed.join("::")
}

impl OrphanModuleShaked {
    pub fn gen_module_name(&self) -> String {
        gen_module_name(&self.rel_rs_path)
    }

    /// Resolve [`OrphanModuleShaked::imported_members`] against rustdoc items
    /// whose `span.filename` matches this module's source file (rooted at
    /// `module_dir_rel_path`).
    ///
    /// - [`OrphanModuleMembers::None`]: empty iterator.
    /// - [`OrphanModuleMembers::All`]: one `Ok(Id)` per public item.
    /// - [`OrphanModuleMembers::Specific`]: one `Ok(Id)` for each resolved
    ///   name, one `Err(name)` for each unresolved name.
    pub fn find_matching_members(
        &self,
        crate_: &rustdoc_types::Crate,
        module_dir_rel_path: &Path,
    ) -> impl Iterator<Item = Result<rustdoc_types::Id, String>> {
        let expected = normalize_path(&module_dir_rel_path.join(&self.rel_rs_path));
        let items_in_module = crate_
            .index
            .values()
            .filter(move |item| {
                item.span
                    .as_ref()
                    .is_some_and(|s| normalize_path(&s.filename) == expected)
            });

        let resolved: Vec<Result<rustdoc_types::Id, String>> = match &self.imported_members {
            OrphanModuleMembers::None => Vec::new(),
            OrphanModuleMembers::All => items_in_module
                .filter(|item| item.visibility == rustdoc_types::Visibility::Public)
                .map(|item| Ok(item.id))
                .collect(),
            OrphanModuleMembers::Specific(names) => {
                let wanted: HashSet<&str> = names.iter().map(String::as_str).collect();
                let mut found_names: HashSet<&str> = HashSet::new();
                let mut out: Vec<Result<rustdoc_types::Id, String>> = Vec::new();
                for item in items_in_module {
                    if let Some(name) = item.name.as_deref() {
                        if let Some(&matched) = wanted.get(name) {
                            out.push(Ok(item.id));
                            found_names.insert(matched);
                        }
                    }
                }
                for name in names {
                    if !found_names.contains(name.as_str()) {
                        out.push(Err(name.clone()));
                    }
                }
                out
            }
        };

        resolved.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustdoc_types::{Crate, Id, Item, ItemEnum, Span, Visibility};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_item(id_num: u32, name: &str, filename: &str, visibility: Visibility) -> (Id, Item) {
        let id = Id(id_num);
        let item = Item {
            id,
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
        };
        (id, item)
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
            target: rustdoc_types::Target {
                triple: String::new(),
                target_features: Vec::new(),
            },
            format_version: 0,
        }
    }

    #[test]
    fn module_name_from_flat_path() {
        assert_eq!(gen_module_name(Path::new("greet.rs")), "greet");
    }

    #[test]
    fn module_name_from_nested_path() {
        assert_eq!(
            gen_module_name(Path::new("nested/deep.rs")),
            "nested::deep"
        );
    }

    #[test]
    fn module_name_from_mod_rs() {
        assert_eq!(gen_module_name(Path::new("utils/mod.rs")), "utils");
    }

    #[test]
    fn none_returns_empty_iter() {
        let module = OrphanModuleShaked {
            rel_rs_path: PathBuf::from("greet.rs"),
            imported_members: OrphanModuleMembers::None,
        };
        let crate_ = make_crate(vec![make_item(
            1,
            "hello",
            "src/greet.rs",
            Visibility::Public,
        )]);

        let out: Vec<_> = module
            .find_matching_members(&crate_, Path::new("src"))
            .collect();
        assert!(out.is_empty());
    }

    #[test]
    fn all_returns_public_items_only() {
        let module = OrphanModuleShaked {
            rel_rs_path: PathBuf::from("math.rs"),
            imported_members: OrphanModuleMembers::All,
        };
        let crate_ = make_crate(vec![
            make_item(1, "add", "src/math.rs", Visibility::Public),
            make_item(2, "sub", "src/math.rs", Visibility::Public),
            make_item(3, "internal", "src/math.rs", Visibility::Default),
            make_item(4, "elsewhere", "src/other.rs", Visibility::Public),
        ]);

        let mut ids: Vec<rustdoc_types::Id> = module
            .find_matching_members(&crate_, Path::new("src"))
            .map(|r| r.unwrap())
            .collect();
        ids.sort_by_key(|id| id.0);
        assert_eq!(ids, vec![Id(1), Id(2)]);
    }

    #[test]
    fn specific_resolves_known_and_reports_missing() {
        let module = OrphanModuleShaked {
            rel_rs_path: PathBuf::from("greet.rs"),
            imported_members: OrphanModuleMembers::Specific(vec![
                "hello".into(),
                "missing".into(),
            ]),
        };
        let crate_ = make_crate(vec![make_item(
            1,
            "hello",
            "src/greet.rs",
            Visibility::Public,
        )]);

        let out: Vec<_> = module
            .find_matching_members(&crate_, Path::new("src"))
            .collect();
        assert_eq!(out.len(), 2);
        assert!(out.contains(&Ok(Id(1))));
        assert!(out.contains(&Err("missing".to_string())));
    }

    #[test]
    fn handles_dotdot_in_span_filenames() {
        let module = OrphanModuleShaked {
            rel_rs_path: PathBuf::from("greet.rs"),
            imported_members: OrphanModuleMembers::All,
        };
        let crate_ = make_crate(vec![make_item(
            1,
            "hello",
            ".rust-curse/../src/greet.rs",
            Visibility::Public,
        )]);

        let ids: Vec<rustdoc_types::Id> = module
            .find_matching_members(&crate_, Path::new("src"))
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(ids, vec![Id(1)]);
    }
}
