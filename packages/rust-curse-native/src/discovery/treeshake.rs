//! Tree-shakes discovered modules against JS imports, keeping only
//! modules that are actually imported and resolving member names to
//! rustdoc item IDs.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::model::{OrphanModule, RsImport, ShakedModule};

pub struct TreeshakeResult {
    pub modules: Vec<ShakedModule>,
    pub warnings: Vec<TreeshakeWarning>,
}

#[derive(Clone, Debug)]
pub enum TreeshakeWarning {
    /// An import references a .rs file that is not a known orphan module.
    UnknownModule {
        rs_rel_path: PathBuf,
        js_abs_paths: Vec<PathBuf>,
    },
    /// An imported member name does not match any public item in the module.
    UnresolvedMember {
        member: String,
        rs_rel_path: PathBuf,
        js_abs_paths: Vec<PathBuf>,
    },
}

impl std::fmt::Display for TreeshakeWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TreeshakeWarning::UnknownModule { rs_rel_path, js_abs_paths } => {
                write!(
                    f,
                    "import of unknown module '{}' in {}",
                    rs_rel_path.display(),
                    format_paths(js_abs_paths),
                )
            }
            TreeshakeWarning::UnresolvedMember { member, rs_rel_path, js_abs_paths } => {
                write!(
                    f,
                    "unresolved member '{}' in module '{}', imported from {}",
                    member,
                    rs_rel_path.display(),
                    format_paths(js_abs_paths),
                )
            }
        }
    }
}

fn format_paths(paths: &[PathBuf]) -> String {
    match paths.len() {
        0 => "(unknown)".to_string(),
        1 => paths[0].display().to_string(),
        n => format!("{} (+{} more)", paths[0].display(), n - 1),
    }
}

/// Matches discovered modules against JS imports.
pub fn treeshake_modules(
    modules: &[OrphanModule],
    imports: &[RsImport],
    crate_: &rustdoc_types::Crate,
) -> TreeshakeResult {
    let grouped = group_imports(imports);
    let mut warnings = Vec::new();

    // Build a set of known module rel_paths for checking unknown imports
    let known_paths: HashSet<&Path> = modules.iter().map(|m| m.rel_path.as_path()).collect();

    for (rs_path, state) in &grouped {
        if !known_paths.contains(rs_path) {
            warnings.push(TreeshakeWarning::UnknownModule {
                rs_rel_path: rs_path.to_path_buf(),
                js_abs_paths: state.js_files.iter().map(|p| p.to_path_buf()).collect(),
            });
        }
    }

    let shaked = modules
        .iter()
        .filter_map(|module| {
            let state = grouped.get(module.rel_path.as_path())?;

            let (imported_item_ids, unresolved) = if state.members.contains("*") {
                (resolve_star_import(&module.item_ids, crate_), Vec::new())
            } else {
                resolve_named_members_with_warnings(&state.members, &module.item_ids, crate_)
            };

            for member in unresolved {
                warnings.push(TreeshakeWarning::UnresolvedMember {
                    member,
                    rs_rel_path: module.rel_path.clone(),
                    js_abs_paths: state.js_files.iter().map(|p| p.to_path_buf()).collect(),
                });
            }

            Some(ShakedModule {
                module: module.clone(),
                importer_abs_paths: state.js_files.iter().map(PathBuf::from).collect(),
                imported_item_ids,
            })
        })
        .collect();

    TreeshakeResult {
        modules: shaked,
        warnings,
    }
}

struct GroupState<'a> {
    js_files: HashSet<&'a Path>,
    members: HashSet<&'a str>,
}

fn group_imports(imports: &[RsImport]) -> HashMap<&Path, GroupState<'_>> {
    let mut by_rs_path: HashMap<&Path, GroupState<'_>> = HashMap::new();

    for import in imports {
        let entry = by_rs_path
            .entry(import.rs_rel_path.as_path())
            .or_insert_with(|| GroupState {
                js_files: HashSet::new(),
                members: HashSet::new(),
            });
        entry.js_files.insert(import.js_abs_path.as_path());
        for m in &import.members {
            entry.members.insert(m.as_str());
        }
    }

    by_rs_path
}

fn resolve_star_import(
    item_ids: &[rustdoc_types::Id],
    crate_: &rustdoc_types::Crate,
) -> Vec<rustdoc_types::Id> {
    item_ids
        .iter()
        .filter(|id| {
            crate_
                .index
                .get(id)
                .is_some_and(|item| item.visibility == rustdoc_types::Visibility::Public)
        })
        .copied()
        .collect()
}

fn resolve_named_members_with_warnings(
    member_names: &HashSet<&str>,
    item_ids: &[rustdoc_types::Id],
    crate_: &rustdoc_types::Crate,
) -> (Vec<rustdoc_types::Id>, Vec<String>) {
    let mut resolved_names: HashSet<&str> = HashSet::new();
    let mut resolved_ids = Vec::new();

    for id in item_ids {
        if let Some(item) = crate_.index.get(id) {
            if let Some(name) = item.name.as_deref() {
                if member_names.contains(name) {
                    resolved_ids.push(*id);
                    resolved_names.insert(name);
                }
            }
        }
    }

    let unresolved: Vec<String> = member_names
        .iter()
        .filter(|name| !resolved_names.contains(*name))
        .map(|name| name.to_string())
        .collect();

    (resolved_ids, unresolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustdoc_types::{Crate, Id, Item, ItemEnum, Visibility};
    use std::collections::HashMap as StdHashMap;

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
                links: StdHashMap::new(),
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

    fn make_test_crate(items: Vec<(Id, Item)>) -> Crate {
        let index: StdHashMap<Id, Item> = items.into_iter().collect();
        Crate {
            root: Id(0),
            crate_version: None,
            includes_private: false,
            index,
            paths: StdHashMap::new(),
            external_crates: StdHashMap::new(),
            format_version: 0,
            target: rustdoc_types::Target {
                triple: String::new(),
                target_features: Vec::new(),
            },
        }
    }

    #[test]
    fn treeshake_filters_unused_modules() {
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

        let imports = vec![RsImport {
            js_abs_path: PathBuf::from("/app/main.js"),
            rs_rel_path: PathBuf::from("greet.rs"),
            members: vec!["hello".into()],
        }];

        let crate_ = make_test_crate(vec![
            make_item(1, "hello", Visibility::Public),
            make_item(2, "unused_fn", Visibility::Public),
        ]);

        let result = treeshake_modules(&modules, &imports, &crate_);

        assert_eq!(result.modules.len(), 1);
        assert_eq!(result.modules[0].module.name, "greet");
        assert_eq!(result.modules[0].imported_item_ids, vec![Id(1)]);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn treeshake_star_import_expands_to_public() {
        let modules = vec![OrphanModule {
            name: "math".into(),
            rel_path: PathBuf::from("math.rs"),
            item_ids: vec![Id(1), Id(2), Id(3)],
        }];

        let imports = vec![RsImport {
            js_abs_path: PathBuf::from("/app/main.js"),
            rs_rel_path: PathBuf::from("math.rs"),
            members: vec!["*".into()],
        }];

        let crate_ = make_test_crate(vec![
            make_item(1, "add", Visibility::Public),
            make_item(2, "sub", Visibility::Public),
            make_item(3, "internal", Visibility::Default),
        ]);

        let result = treeshake_modules(&modules, &imports, &crate_);

        assert_eq!(result.modules.len(), 1);
        assert_eq!(result.modules[0].imported_item_ids.len(), 2);
        assert!(result.modules[0].imported_item_ids.contains(&Id(1)));
        assert!(result.modules[0].imported_item_ids.contains(&Id(2)));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn treeshake_merges_importers() {
        let modules = vec![OrphanModule {
            name: "greet".into(),
            rel_path: PathBuf::from("greet.rs"),
            item_ids: vec![Id(1), Id(2)],
        }];

        let imports = vec![
            RsImport {
                js_abs_path: PathBuf::from("/app/a.js"),
                rs_rel_path: PathBuf::from("greet.rs"),
                members: vec!["hello".into()],
            },
            RsImport {
                js_abs_path: PathBuf::from("/app/b.js"),
                rs_rel_path: PathBuf::from("greet.rs"),
                members: vec!["goodbye".into()],
            },
        ];

        let crate_ = make_test_crate(vec![
            make_item(1, "hello", Visibility::Public),
            make_item(2, "goodbye", Visibility::Public),
        ]);

        let result = treeshake_modules(&modules, &imports, &crate_);

        assert_eq!(result.modules.len(), 1);
        assert_eq!(result.modules[0].importer_abs_paths.len(), 2);
        assert_eq!(result.modules[0].imported_item_ids.len(), 2);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn treeshake_warns_on_unresolved_member() {
        let modules = vec![OrphanModule {
            name: "greet".into(),
            rel_path: PathBuf::from("greet.rs"),
            item_ids: vec![Id(1)],
        }];

        let imports = vec![RsImport {
            js_abs_path: PathBuf::from("/app/main.js"),
            rs_rel_path: PathBuf::from("greet.rs"),
            members: vec!["hello".into(), "nonexistent".into()],
        }];

        let crate_ = make_test_crate(vec![make_item(1, "hello", Visibility::Public)]);

        let result = treeshake_modules(&modules, &imports, &crate_);

        assert_eq!(result.modules.len(), 1);
        assert_eq!(result.modules[0].imported_item_ids, vec![Id(1)]);
        assert_eq!(result.warnings.len(), 1);
        match &result.warnings[0] {
            TreeshakeWarning::UnresolvedMember {
                member,
                rs_rel_path,
                ..
            } => {
                assert_eq!(member, "nonexistent");
                assert_eq!(rs_rel_path, &PathBuf::from("greet.rs"));
            }
            other => panic!("expected UnresolvedMember, got {:?}", other),
        }
    }

    #[test]
    fn treeshake_warns_on_unknown_module() {
        let modules = vec![OrphanModule {
            name: "greet".into(),
            rel_path: PathBuf::from("greet.rs"),
            item_ids: vec![Id(1)],
        }];

        let imports = vec![RsImport {
            js_abs_path: PathBuf::from("/app/main.js"),
            rs_rel_path: PathBuf::from("missing.rs"),
            members: vec!["foo".into()],
        }];

        let crate_ = make_test_crate(vec![make_item(1, "hello", Visibility::Public)]);

        let result = treeshake_modules(&modules, &imports, &crate_);

        assert!(result.modules.is_empty());
        assert_eq!(result.warnings.len(), 1);
        match &result.warnings[0] {
            TreeshakeWarning::UnknownModule {
                rs_rel_path,
                js_abs_paths,
            } => {
                assert_eq!(rs_rel_path, &PathBuf::from("missing.rs"));
                assert_eq!(js_abs_paths, &vec![PathBuf::from("/app/main.js")]);
            }
            other => panic!("expected UnknownModule, got {:?}", other),
        }
    }
}
