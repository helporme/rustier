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

pub use resolve::gen_module_name;

#[derive(Debug)]
pub struct OrphanScanOpts<'a> {
    /// Absolute path of the directory under which orphan `.rs` modules are
    /// discovered.
    pub root_abs_path: &'a Path,
    /// Filters applied to discovered `.rs` orphans.
    pub rs_filter: SourceFilter<'a>,
}

#[derive(Debug)]
pub struct OrphanShakeOpts<'a> {
    /// Absolute path of the directory under which orphan `.rs` modules live.
    /// JS specifiers like `'../src/greet.rs'` resolve relative to this
    /// directory.
    pub root_abs_path: &'a Path,
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
    /// Path to the `.rs` file relative to `OrphanScanOpts.root_abs_path`,
    /// e.g. `greet.rs`, `nested/deep.rs`, `utils/mod.rs`.
    pub rel_rs_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct OrphanModuleShaked {
    /// Path to the `.rs` file relative to the module root.
    pub rel_rs_path: PathBuf,
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

/// Discover orphan rust modules under `opts.root_abs_path`.
pub fn scan(opts: OrphanScanOpts<'_>) -> Result<Vec<OrphanModule>, OrphanScanError> {
    let abs_root =
        fs::canonicalize(opts.root_abs_path).map_err(|source| OrphanScanError::RootNotFound {
            abs_path: opts.root_abs_path.to_path_buf(),
            source,
        })?;

    let mut rel_paths = discover::walk_orphans(&abs_root)?;
    apply_rs_filter(&mut rel_paths, &opts.rs_filter)?;
    detect_conflicts(&rel_paths)?;

    rel_paths.sort();
    Ok(rel_paths
        .into_iter()
        .map(|rel_rs_path| OrphanModule { rel_rs_path })
        .collect())
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
    let abs_root =
        fs::canonicalize(opts.root_abs_path).map_err(|source| OrphanScanError::RootNotFound {
            abs_path: opts.root_abs_path.to_path_buf(),
            source,
        })?;

    let raw_imports = imports::scan_js_imports(&abs_root, &opts.js_filter)?;
    let mut buckets: BTreeMap<PathBuf, Bucket> = BTreeMap::new();
    for import in raw_imports {
        if import.members.iter().any(|m| m == "default") {
            return Err(OrphanScanError::DefaultImport {
                rel_rs_path: import.rel_rs_path,
            });
        }
        let entry = buckets.entry(import.rel_rs_path).or_default();
        for m in import.members {
            if m == "*" {
                entry.star = true;
            } else {
                entry.members.insert(m);
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
    rel_paths: &mut Vec<PathBuf>,
    filter: &SourceFilter<'_>,
) -> Result<(), OrphanScanError> {
    let include = compile_patterns(filter.include)?;
    let exclude = compile_patterns(filter.exclude)?;

    rel_paths.retain(|rel| {
        let s = rel.to_string_lossy();
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

fn detect_conflicts(rel_paths: &[PathBuf]) -> Result<(), OrphanScanError> {
    let mut by_name: BTreeMap<String, &PathBuf> = BTreeMap::new();
    for rel in rel_paths {
        let name = gen_module_name(rel);
        if let Some(prev) = by_name.insert(name.clone(), rel) {
            return Err(OrphanScanError::Conflict {
                name,
                rel_path_a: prev.clone(),
                rel_path_b: rel.clone(),
            });
        }
    }
    Ok(())
}
