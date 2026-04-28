//! Module-name derivation and rustdoc member resolution for [`OrphanModule`].

use std::collections::HashSet;
use std::path::Path;

use crate::common::normalize_path;

use super::{OrphanModuleMembers, OrphanModuleShaked};

/// Derive the `::`-separated rust module name from a relative `.rs` path.
///
/// Each path segment is sanitized into a valid rust identifier: characters
/// other than ASCII alphanumerics or `_` are replaced with `_`, and a
/// leading digit is prefixed with `_`. Conflicts arising from sanitization
/// (e.g. `my-mod/` and `my_mod/` both becoming `my_mod`) are caught later
/// by [`super::detect_conflicts`].
pub(super) fn gen_module_name(rel_rs_path: &Path) -> String {
    let without_ext = rel_rs_path.with_extension("");

    let segments: Vec<String> = without_ext
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .map(sanitize_segment)
        .collect();

    let trimmed = if segments.last().map(String::as_str) == Some("mod") {
        &segments[..segments.len() - 1]
    } else {
        &segments
    };

    trimmed.join("::")
}

fn sanitize_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    let mut chars = segment.chars();
    if let Some(first) = chars.next() {
        if first.is_ascii_digit() {
            out.push('_');
            out.push(first);
        } else if first.is_ascii_alphabetic() || first == '_' {
            out.push(first);
        } else {
            out.push('_');
        }
    }
    for c in chars {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out
}

impl OrphanModuleShaked {
    /// Resolve [`OrphanModuleShaked::imported_members`] against rustdoc items
    /// whose `span.filename` matches this module's source file.
    ///
    /// - [`OrphanModuleMembers::None`]: empty iterator.
    /// - [`OrphanModuleMembers::All`]: one `Ok(Id)` per public item.
    /// - [`OrphanModuleMembers::Specific`]: one `Ok(Id)` for each resolved
    ///   name, one `Err(name)` for each unresolved name.
    pub fn find_matching_members(
        &self,
        crate_: &rustdoc_types::Crate,
    ) -> impl Iterator<Item = Result<rustdoc_types::Id, String>> {
        let expected = normalize_path(&self.rel_rs_path);
        let items_in_module = crate_.index.values().filter(move |item| {
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
        assert_eq!(gen_module_name(Path::new("nested/deep.rs")), "nested::deep");
    }

    #[test]
    fn module_name_from_mod_rs() {
        assert_eq!(gen_module_name(Path::new("utils/mod.rs")), "utils");
    }

    #[test]
    fn module_name_segment_starting_with_digit_gets_underscore_prefix() {
        assert_eq!(gen_module_name(Path::new("2024/log.rs")), "_2024::log");
        assert_eq!(gen_module_name(Path::new("9foo.rs")), "_9foo");
    }

    #[test]
    fn module_name_replaces_extra_dots_in_filename() {
        assert_eq!(gen_module_name(Path::new("foo.bar.rs")), "foo_bar");
        assert_eq!(
            gen_module_name(Path::new("nested/file.local.rs")),
            "nested::file_local"
        );
    }

    #[test]
    fn module_name_replaces_hyphens_and_other_invalid_chars() {
        assert_eq!(gen_module_name(Path::new("my-mod/baz.rs")), "my_mod::baz");
        assert_eq!(
            gen_module_name(Path::new("weird name/x.rs")),
            "weird_name::x"
        );
    }

    #[test]
    fn none_returns_empty_iter() {
        let module = OrphanModuleShaked {
            rel_rs_path: PathBuf::from("src/greet.rs"),
            mod_name: "greet".into(),
            imported_members: OrphanModuleMembers::None,
        };
        let crate_ = make_crate(vec![make_item(
            1,
            "hello",
            "src/greet.rs",
            Visibility::Public,
        )]);

        let out: Vec<_> = module.find_matching_members(&crate_).collect();
        assert!(out.is_empty());
    }

    #[test]
    fn all_returns_public_items_only() {
        let module = OrphanModuleShaked {
            rel_rs_path: PathBuf::from("src/math.rs"),
            mod_name: "math".into(),
            imported_members: OrphanModuleMembers::All,
        };
        let crate_ = make_crate(vec![
            make_item(1, "add", "src/math.rs", Visibility::Public),
            make_item(2, "sub", "src/math.rs", Visibility::Public),
            make_item(3, "internal", "src/math.rs", Visibility::Default),
            make_item(4, "elsewhere", "src/other.rs", Visibility::Public),
        ]);

        let mut ids: Vec<rustdoc_types::Id> = module
            .find_matching_members(&crate_)
            .map(|r| r.unwrap())
            .collect();
        ids.sort_by_key(|id| id.0);
        assert_eq!(ids, vec![Id(1), Id(2)]);
    }

    #[test]
    fn specific_resolves_known_and_reports_missing() {
        let module = OrphanModuleShaked {
            rel_rs_path: PathBuf::from("src/greet.rs"),
            mod_name: "greet".into(),
            imported_members: OrphanModuleMembers::Specific(vec!["hello".into(), "missing".into()]),
        };
        let crate_ = make_crate(vec![make_item(
            1,
            "hello",
            "src/greet.rs",
            Visibility::Public,
        )]);

        let out: Vec<_> = module.find_matching_members(&crate_).collect();
        assert_eq!(out.len(), 2);
        assert!(out.contains(&Ok(Id(1))));
        assert!(out.contains(&Err("missing".to_string())));
    }

    #[test]
    fn handles_dotdot_in_span_filenames() {
        let module = OrphanModuleShaked {
            rel_rs_path: PathBuf::from("src/greet.rs"),
            mod_name: "greet".into(),
            imported_members: OrphanModuleMembers::All,
        };
        let crate_ = make_crate(vec![make_item(
            1,
            "hello",
            ".rust-curse/../src/greet.rs",
            Visibility::Public,
        )]);

        let ids: Vec<rustdoc_types::Id> = module
            .find_matching_members(&crate_)
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(ids, vec![Id(1)]);
    }
}
