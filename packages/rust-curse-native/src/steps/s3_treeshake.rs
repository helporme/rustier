//! Step 3.
//! Tree-shakes discovered modules against JS imports, keeping only
//! modules that are actually imported and tracking which members are used.

use std::collections::{HashMap, HashSet};

use crate::steps::{s1_discovery::RustModule, s2_import_scan::RustImport};

pub struct RustModuleShaked<'a> {
    pub module: &'a RustModule,
    /// JS file paths that import this module
    pub importer_paths: Vec<String>,
    /// Member names that were imported from at least one JS file
    pub imported_members: Vec<String>,
}

/// Matches discovered modules against JS imports.
/// Returns only modules that are referenced by at least one import,
/// with the set of imported member names and importer paths.
pub fn treeshake<'a>(
    modules: &'a [RustModule],
    imports: &[RustImport],
) -> Vec<RustModuleShaked<'a>> {
    // Group imports by rs_file path
    let mut by_rs_file: HashMap<&str, GroupState> = HashMap::new();

    for import in imports {
        let entry = by_rs_file
            .entry(&import.rs_file)
            .or_insert_with(|| GroupState {
                js_files: HashSet::new(),
                members: HashSet::new(),
            });
        entry.js_files.insert(&import.js_file);
        for m in &import.members {
            entry.members.insert(m.as_str());
        }
    }

    modules
        .iter()
        .filter_map(|module| {
            let state = by_rs_file.get(module.path.as_str())?;

            let imported_members: Vec<String> = if state.members.contains("*") {
                // Star import: all top-level fn members are considered imported
                module
                    .members
                    .as_ref()
                    .map(|members| {
                        members
                            .iter()
                            .filter_map(|m| match m {
                                crate::steps::s1_discovery::RustMember::Fn(f) => {
                                    // Extract the short name (last segment)
                                    Some(short_name(&f.name).to_string())
                                }
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                state.members.iter().map(|s| s.to_string()).collect()
            };

            Some(RustModuleShaked {
                module,
                importer_paths: state.js_files.iter().map(|s| s.to_string()).collect(),
                imported_members,
            })
        })
        .collect()
}

struct GroupState<'a> {
    js_files: HashSet<&'a str>,
    members: HashSet<&'a str>,
}

fn short_name(full_name: &str) -> &str {
    full_name.rsplit("::").next().unwrap_or(full_name)
}
