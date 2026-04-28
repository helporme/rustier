//! Discovery of orphan rust modules and resolution of which members are
//! imported by JS/TS code.
//!
//! An orphan module is a standalone rust module that is not declared as part
//! of another module's `mod` tree. [`scan`] walks the rust source tree to
//! find them. [`shake`] folds those modules with the JS/TS imports that
//! reference them, producing per-target [`OrphanModuleShaked`] entries.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

mod discover;
mod imports;
mod resolve;

#[derive(Debug)]
pub struct OrphanScanOpts<'a> {
    /// Absolute path of the project root.
    pub project_root_abs_path: &'a Path,
    /// Module roots, each relative to `project_root_abs_path`. Each root is
    /// walked independently for orphan `.rs` modules; the rust module name
    /// of a discovered file is derived from its path within its containing
    /// module root.
    pub mod_root_rel_paths: &'a [PathBuf],
    /// Filters applied to discovered `.rs` orphans, matched against paths
    /// relative to the project root.
    pub rs_filter: SourceFilter<'a>,
}

#[derive(Debug)]
pub struct OrphanShakeOpts<'a> {
    /// Absolute path of the project root.
    pub project_root_abs_path: &'a Path,
    /// Module roots, each relative to `project_root_abs_path`. JS specifiers
    /// like `'../src/greet.rs'` resolve to a `.rs` file under one of these
    /// roots.
    pub mod_root_rel_paths: &'a [PathBuf],
    /// Filters selecting which JS/TS files are scanned for imports.
    pub js_filter: SourceFilter<'a>,
}

#[derive(Debug, Default)]
pub struct SourceFilter<'a> {
    /// Glob patterns to include.
    pub include: &'a [String],
    /// Glob patterns to exclude.
    pub exclude: &'a [String],
}

#[derive(Clone, Debug)]
pub struct OrphanModule {
    /// Path to the `.rs` file relative to the project root,
    /// e.g. `src/greet.rs`, `src/nested/deep.rs`, `generated/src/utils/mod.rs`.
    pub rel_rs_path: PathBuf,
    /// Rust module name derived from the path within the containing module
    pub mod_name: String,
}

#[derive(Clone, Debug)]
pub struct OrphanModuleShaked {
    /// Path to the `.rs` file relative to the project root.
    pub rel_rs_path: PathBuf,
    /// Rust module name derived from the path within the containing module
    /// root.
    pub mod_name: String,
    /// Members imported from at least one JS/TS file.
    pub imported_members: OrphanModuleMembers,
}

#[derive(Clone, Debug)]
pub enum OrphanModuleMembers {
    /// Star import seen anywhere; surface all public items.
    All,
    /// Specific names imported.
    Specific(Vec<String>),
    /// Module discovered but never imported by this target.
    None,
}

#[derive(Debug, thiserror::Error)]
pub enum OrphanScanError {
    #[error("root folder '{}' not found", abs_path.display())]
    RootNotFound {
        abs_path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("invalid glob pattern '{pattern}'")]
    InvalidGlob {
        pattern: String,
        #[source]
        source: glob::PatternError,
    },

    #[error("failed to read directory '{}'", abs_path.display())]
    ReadDir {
        abs_path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to read file '{}'", abs_path.display())]
    ReadFile {
        abs_path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("conflicting module '{name}': both '{}' and '{}' exist", rel_path_a.display(), rel_path_b.display())]
    Conflict {
        name: String,
        rel_path_a: PathBuf,
        rel_path_b: PathBuf,
    },

    // todo: failing the whole scan beacause of this seems silly
    #[error(
        "default import of '{}' is not supported; rust modules have no default export, \
         use named imports like `import {{ name }} from '...'`",
        rel_rs_path.display()
    )]
    DefaultImport { rel_rs_path: PathBuf },
}

/// Discover orphan rust modules under each entry of `opts.mod_root_rel_paths`.
pub fn scan(opts: OrphanScanOpts<'_>) -> Result<Vec<OrphanModule>, OrphanScanError> {
    let abs_project_root = fs::canonicalize(opts.project_root_abs_path).map_err(|source| {
        OrphanScanError::RootNotFound {
            abs_path: opts.project_root_abs_path.to_path_buf(),
            source,
        }
    })?;

    let mut modules: Vec<OrphanModule> = Vec::new();
    for mod_root_rel in opts.mod_root_rel_paths {
        let abs_mod_root = abs_project_root.join(mod_root_rel);
        let abs_mod_root =
            fs::canonicalize(&abs_mod_root).map_err(|source| OrphanScanError::RootNotFound {
                abs_path: abs_mod_root,
                source,
            })?;

        for rel_within_mod_root in discover::walk_orphans(&abs_mod_root)? {
            let mod_name = resolve::gen_module_name(&rel_within_mod_root);
            let rel_rs_path = mod_root_rel.join(&rel_within_mod_root);
            modules.push(OrphanModule {
                rel_rs_path,
                mod_name,
            });
        }
    }

    apply_rs_filter(&mut modules, &opts.rs_filter)?;
    detect_conflicts(&modules)?;

    modules.sort_by(|a, b| a.rel_rs_path.cmp(&b.rel_rs_path));
    Ok(modules)
}

/// Join a discovered set of orphan modules with the JS/TS imports referencing
/// them. Returns one [`OrphanModuleShaked`] per input module, with
/// `imported_members` reflecting what JS code in `opts.js_filter` actually
/// pulls in. Imports resolving to a path not present in `modules` are
/// silently dropped.
pub fn shake(
    modules: &[OrphanModule],
    opts: OrphanShakeOpts<'_>,
) -> Result<Vec<OrphanModuleShaked>, OrphanScanError> {
    let abs_project_root = fs::canonicalize(opts.project_root_abs_path).map_err(|source| {
        OrphanScanError::RootNotFound {
            abs_path: opts.project_root_abs_path.to_path_buf(),
            source,
        }
    })?;

    let mut buckets: BTreeMap<PathBuf, Bucket> = BTreeMap::new();
    for mod_root_rel in opts.mod_root_rel_paths {
        let abs_mod_root = abs_project_root.join(mod_root_rel);
        let abs_mod_root =
            fs::canonicalize(&abs_mod_root).map_err(|source| OrphanScanError::RootNotFound {
                abs_path: abs_mod_root,
                source,
            })?;

        for import in imports::scan_js_imports(&abs_mod_root, &opts.js_filter)? {
            let rel_rs_path = mod_root_rel.join(&import.rel_rs_path);
            if import.members.iter().any(|m| m == "default") {
                return Err(OrphanScanError::DefaultImport { rel_rs_path });
            }
            let entry = buckets.entry(rel_rs_path).or_default();
            for m in import.members {
                if m == "*" {
                    entry.star = true;
                } else {
                    entry.members.insert(m);
                }
            }
        }
    }

    Ok(modules
        .iter()
        .map(|module| {
            let imported_members = match buckets.remove(&module.rel_rs_path) {
                None => OrphanModuleMembers::None,
                Some(bucket) if bucket.star => OrphanModuleMembers::All,
                Some(bucket) => OrphanModuleMembers::Specific(bucket.members.into_iter().collect()),
            };
            OrphanModuleShaked {
                rel_rs_path: module.rel_rs_path.clone(),
                mod_name: module.mod_name.clone(),
                imported_members,
            }
        })
        .collect())
}

#[derive(Default)]
struct Bucket {
    star: bool,
    members: BTreeSet<String>,
}

fn apply_rs_filter(
    modules: &mut Vec<OrphanModule>,
    filter: &SourceFilter<'_>,
) -> Result<(), OrphanScanError> {
    let include = compile_patterns(filter.include)?;
    let exclude = compile_patterns(filter.exclude)?;

    modules.retain(|module| {
        let s = module.rel_rs_path.to_string_lossy();
        let included = include.is_empty() || include.iter().any(|p| p.matches(&s));
        let excluded = exclude.iter().any(|p| p.matches(&s));
        included && !excluded
    });
    Ok(())
}

fn compile_patterns(patterns: &[String]) -> Result<Vec<glob::Pattern>, OrphanScanError> {
    patterns
        .iter()
        .map(|p| {
            glob::Pattern::new(p).map_err(|source| OrphanScanError::InvalidGlob {
                pattern: p.clone(),
                source,
            })
        })
        .collect()
}

fn detect_conflicts(modules: &[OrphanModule]) -> Result<(), OrphanScanError> {
    let mut by_name: BTreeMap<&str, &PathBuf> = BTreeMap::new();
    for module in modules {
        if let Some(prev) = by_name.insert(&module.mod_name, &module.rel_rs_path) {
            return Err(OrphanScanError::Conflict {
                name: module.mod_name.clone(),
                rel_path_a: prev.clone(),
                rel_path_b: module.rel_rs_path.clone(),
            });
        }
    }
    Ok(())
}
