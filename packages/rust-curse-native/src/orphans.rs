//! Discovery of orphan rust modules.
//!
//! An orphan module is a standalone rust module that is not declared as part
//! of another module's `mod` tree. [`scan`] walks the rust source tree to
//! find them.
//!
//! Examples:
//! - `src/greet.rs` -> orphan module `greet`
//! - `src/nested/deep.rs` -> orphan module `nested::deep`

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct OrphanScanOpts<'a> {
    /// Absolute path of the project root.
    pub project_root_abs_path: &'a Path,
    /// Source root, relative to `project_root_abs_path`. Walked for orphan
    /// `.rs` modules; the rust module name of a discovered file is derived
    /// from its path within this root.
    pub src_rel_path: &'a Path,
    /// Filters applied to discovered `.rs` orphans, matched against paths
    /// relative to the project root.
    pub rs_filter: SourceFilter<'a>,
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
    /// e.g. `src/greet.rs` or `src/nested/deep.rs`.
    pub rel_rs_path: PathBuf,
    /// Rust module name derived from the path within the containing module
    pub mod_name: String,
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

    #[error("conflicting module '{name}': both '{}' and '{}' exist", rel_path_a.display(), rel_path_b.display())]
    Conflict {
        name: String,
        rel_path_a: PathBuf,
        rel_path_b: PathBuf,
    },

    #[error(
        "legacy 'mod.rs' layout is not supported at '{}'; use the modern layout (file.rs + file/ submodules)",
        rel_path.display()
    )]
    LegacyModRs { rel_path: PathBuf },
}

/// Discover orphan rust modules under `opts.src_rel_path`.
pub fn scan(opts: OrphanScanOpts<'_>) -> Result<Vec<OrphanModule>, OrphanScanError> {
    let abs_project_root = fs::canonicalize(opts.project_root_abs_path).map_err(|source| {
        OrphanScanError::RootNotFound {
            abs_path: opts.project_root_abs_path.to_path_buf(),
            source,
        }
    })?;

    let abs_src_root = abs_project_root.join(opts.src_rel_path);
    let abs_src_root =
        fs::canonicalize(&abs_src_root).map_err(|source| OrphanScanError::RootNotFound {
            abs_path: abs_src_root,
            source,
        })?;

    let mut modules: Vec<OrphanModule> = Vec::new();
    for rel_within_src_root in walk_orphans(&abs_src_root)? {
        let mod_name = gen_module_name(&rel_within_src_root);
        let rel_rs_path = opts.src_rel_path.join(&rel_within_src_root);
        modules.push(OrphanModule {
            rel_rs_path,
            mod_name,
        });
    }

    apply_rs_filter(&mut modules, &opts.rs_filter)?;
    detect_conflicts(&modules)?;

    modules.sort_by(|a, b| a.rel_rs_path.cmp(&b.rel_rs_path));
    Ok(modules)
}

fn walk_orphans(abs_root: &Path) -> Result<Vec<PathBuf>, OrphanScanError> {
    let mut rel_paths = Vec::new();
    walk_dir(abs_root, abs_root, true, &mut rel_paths)?;
    Ok(rel_paths)
}

fn walk_dir(
    abs_dir: &Path,
    abs_root: &Path,
    collect_orphans: bool,
    out: &mut Vec<PathBuf>,
) -> Result<(), OrphanScanError> {
    let entries = fs::read_dir(abs_dir).map_err(|source| OrphanScanError::ReadDir {
        abs_path: abs_dir.to_path_buf(),
        source,
    })?;

    let mut rs_files: Vec<PathBuf> = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let abs_path = entry.path();
        if abs_path.is_file() && abs_path.extension().is_some_and(|e| e == "rs") {
            rs_files.push(abs_path);
        } else if abs_path.is_dir() {
            dirs.push(abs_path);
        }
    }

    for abs_path in &rs_files {
        let stem = abs_path.file_stem().unwrap().to_string_lossy();
        if stem == "mod" {
            let rel_path = abs_path.strip_prefix(abs_root).unwrap().to_path_buf();
            return Err(OrphanScanError::LegacyModRs { rel_path });
        }
        if collect_orphans {
            let rel_path = abs_path.strip_prefix(abs_root).unwrap().to_path_buf();
            out.push(rel_path);
        }
    }

    let rs_stems: HashSet<String> = rs_files
        .iter()
        .filter_map(|p| Some(p.file_stem()?.to_string_lossy().into_owned()))
        .collect();

    for abs_path in &dirs {
        let dir_name = abs_path.file_name().unwrap().to_string_lossy();

        // In a modern-layout submodule subtree (file.rs + file/), submodules
        // are owned by file.rs and are not orphans, but we still recurse to
        // reject any legacy `mod.rs` files.
        let recurse_collect = collect_orphans && !rs_stems.contains(dir_name.as_ref());
        walk_dir(abs_path, abs_root, recurse_collect, out)?;
    }

    Ok(())
}

/// Derive the `::`-separated rust module name from a relative `.rs` path.
///
/// Each path segment is sanitized into a valid rust identifier: characters
/// other than ASCII alphanumerics or `_` are replaced with `_`, and a
/// leading digit is prefixed with `_`. Conflicts arising from sanitization
/// (e.g. `my-mod/` and `my_mod/` both becoming `my_mod`) are caught later
/// by [`detect_conflicts`].
pub fn gen_module_name(rel_rs_path: &Path) -> String {
    let without_ext = rel_rs_path.with_extension("");

    let segments: Vec<String> = without_ext
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .map(sanitize_segment)
        .collect();

    segments.join("::")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
        paths.sort();
        paths
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
    fn discovers_orphan_modules() {
        let dir = tempfile::tempdir().unwrap();

        fs::write(dir.path().join("greet.rs"), "pub fn hello() {}").unwrap();
        fs::write(dir.path().join("math.rs"), "pub fn add() {}").unwrap();

        fs::create_dir_all(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("nested/deep.rs"), "pub fn go() {}").unwrap();

        fs::write(dir.path().join("parent.rs"), "pub mod child;").unwrap();
        fs::create_dir_all(dir.path().join("parent")).unwrap();
        fs::write(dir.path().join("parent/child.rs"), "").unwrap();

        let abs_root = fs::canonicalize(dir.path()).unwrap();
        let paths = sorted(walk_orphans(&abs_root).unwrap());

        assert_eq!(
            paths,
            vec![
                PathBuf::from("greet.rs"),
                PathBuf::from("math.rs"),
                PathBuf::from("nested/deep.rs"),
                PathBuf::from("parent.rs"),
            ]
        );
    }

    #[test]
    fn standalone_mod_rs_errors() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("mod.rs"), "").unwrap();
        fs::write(dir.path().join("real.rs"), "").unwrap();

        let abs_root = fs::canonicalize(dir.path()).unwrap();
        let err = walk_orphans(&abs_root).unwrap_err();

        assert!(
            matches!(
                err,
                OrphanScanError::LegacyModRs { ref rel_path } if rel_path == &PathBuf::from("mod.rs")
            ),
            "{err:?}"
        );
    }

    #[test]
    fn nested_mod_rs_errors() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("utils")).unwrap();
        fs::write(dir.path().join("utils/mod.rs"), "").unwrap();

        let abs_root = fs::canonicalize(dir.path()).unwrap();
        let err = walk_orphans(&abs_root).unwrap_err();

        assert!(
            matches!(
                err,
                OrphanScanError::LegacyModRs { ref rel_path } if rel_path == &PathBuf::from("utils/mod.rs")
            ),
            "{err:?}"
        );
    }

    #[test]
    fn handles_dotted_filenames() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.local.rs"), "").unwrap();
        fs::write(dir.path().join("simple.rs"), "").unwrap();

        let abs_root = fs::canonicalize(dir.path()).unwrap();
        let paths = sorted(walk_orphans(&abs_root).unwrap());

        assert_eq!(
            paths,
            vec![PathBuf::from("file.local.rs"), PathBuf::from("simple.rs")]
        );
    }

    #[test]
    fn modern_layout_skips_submodule_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("parent.rs"), "pub mod child;").unwrap();
        fs::create_dir_all(dir.path().join("parent")).unwrap();
        fs::write(dir.path().join("parent/child.rs"), "").unwrap();
        fs::write(dir.path().join("other.rs"), "").unwrap();

        let abs_root = fs::canonicalize(dir.path()).unwrap();
        let paths = sorted(walk_orphans(&abs_root).unwrap());

        assert_eq!(
            paths,
            vec![PathBuf::from("other.rs"), PathBuf::from("parent.rs"),]
        );
    }
}
