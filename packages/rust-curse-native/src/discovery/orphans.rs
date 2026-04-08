//! Filesystem walk to discover orphan rust modules.
//!
//! An orphan module is a standalone `.rs` file (or `folder/mod.rs`) that is
//! not declared as part of another module's `mod` tree.
//!
//! Examples:
//! - `src/greet.rs` → orphan module `greet`
//! - `src/utils/mod.rs` → orphan module `utils`
//! - `src/nested/deep.rs` → orphan module `nested::deep`
//!
//! Directories with a `mod.rs` are treated as a single module (not recursed into).
//! Directories without `mod.rs` are recursed into to find deeper orphans.

use std::fs;
use std::path::{Path, PathBuf};

use crate::model::OrphanModule;

/// Discover all orphan rust modules under `abs_root`.
pub fn discover_orphans(abs_root: &Path) -> Result<Vec<OrphanModule>, DiscoverError> {
    let abs_root = fs::canonicalize(abs_root)
        .map_err(|e| DiscoverError::RootNotFound(abs_root.to_path_buf(), e))?;

    let mut result = Vec::new();
    discover_recursive(&abs_root, &abs_root, &mut result)?;
    result.sort_by(|a, b| a.name.cmp(&b.name));

    // Check for duplicate module names (e.g. abc.rs and abc/mod.rs)
    for window in result.windows(2) {
        if window[0].name == window[1].name {
            return Err(DiscoverError::Conflict {
                name: window[0].name.clone(),
                rel_path_a: window[0].rel_path.clone(),
                rel_path_b: window[1].rel_path.clone(),
            });
        }
    }

    Ok(result)
}

fn discover_recursive(
    abs_dir: &Path,
    abs_root: &Path,
    result: &mut Vec<OrphanModule>,
) -> Result<(), DiscoverError> {
    let entries =
        fs::read_dir(abs_dir).map_err(|e| DiscoverError::ReadDir(abs_dir.to_path_buf(), e))?;

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

    // Collect .rs file stems so we can detect the modern layout
    // (file.rs alongside file/ directory)
    let rs_stems: std::collections::HashSet<String> = rs_files
        .iter()
        .filter_map(|p| {
            let stem = p.file_stem()?.to_string_lossy().into_owned();
            if stem == "mod" {
                None
            } else {
                Some(stem)
            }
        })
        .collect();

    for abs_path in &rs_files {
        let stem = abs_path.file_stem().unwrap().to_string_lossy();
        if stem == "mod" {
            continue;
        }
        let rel_path = abs_path.strip_prefix(abs_root).unwrap().to_path_buf();
        let name = build_module_name(&rel_path);
        result.push(OrphanModule { name, rel_path });
    }

    for abs_path in &dirs {
        let dir_name = abs_path.file_name().unwrap().to_string_lossy();
        let abs_mod_file = abs_path.join("mod.rs");

        if abs_mod_file.exists() {
            // Old layout: dir/mod.rs
            let rel_path = abs_mod_file.strip_prefix(abs_root).unwrap().to_path_buf();
            let name = build_module_name(&rel_path);
            result.push(OrphanModule { name, rel_path });
        } else if rs_stems.contains(dir_name.as_ref()) {
            // Modern layout: file.rs alongside file/ directory.
            // Submodules in file/ are handled by Rust's module system, skip.
        } else {
            // No mod.rs and no sibling .rs file -> recurse for deeper orphans
            discover_recursive(abs_path, abs_root, result)?;
        }
    }

    Ok(())
}

/// Build a `::` separated module name from a relative file path.
///
/// `foo/bar.rs` → `foo::bar`
/// `foo/mod.rs` → `foo`
fn build_module_name(rel_path: &Path) -> String {
    let without_ext = rel_path.with_extension("");

    let segments: Vec<&str> = without_ext
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    // Trim trailing "mod" for mod.rs files
    let trimmed = if segments.last() == Some(&"mod") {
        &segments[..segments.len() - 1]
    } else {
        &segments
    };

    trimmed.join("::")
}

#[derive(Debug, thiserror::Error)]
pub enum DiscoverError {
    #[error("root folder '{}' not found", .0.display())]
    RootNotFound(PathBuf, #[source] std::io::Error),

    #[error("failed to read directory '{}'", .0.display())]
    ReadDir(PathBuf, #[source] std::io::Error),

    #[error("conflicting module '{}': both '{}' and '{}' exist", .name, .rel_path_a.display(), .rel_path_b.display())]
    Conflict {
        name: String,
        rel_path_a: PathBuf,
        rel_path_b: PathBuf,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_temp_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();

        // src/greet.rs
        fs::write(dir.path().join("greet.rs"), "pub fn hello() {}").unwrap();
        fs::write(dir.path().join("greet.js"), "const hello = 'kitty';").unwrap();

        // src/math.rs
        fs::write(dir.path().join("math.rs"), "pub fn add() {}").unwrap();

        // src/utils/mod.rs (directory module)
        fs::create_dir_all(dir.path().join("utils")).unwrap();
        fs::write(dir.path().join("utils/mod.rs"), "pub fn help() {}").unwrap();
        fs::write(dir.path().join("utils/mod.js"), "const hello = 'kitty';").unwrap();

        // src/nested/deep.rs (no mod.rs in nested/ -> recurse)
        fs::create_dir_all(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("nested/deep.rs"), "pub fn go() {}").unwrap();

        // src/parent.rs + src/parent/child.rs (modern layout -> skip submodules)
        fs::write(dir.path().join("parent.rs"), "pub mod child;").unwrap();
        fs::create_dir_all(dir.path().join("parent")).unwrap();
        fs::write(dir.path().join("parent/child.rs"), "").unwrap();

        dir
    }

    #[test]
    fn discovers_orphan_modules() {
        let dir = setup_temp_dir();
        let modules = discover_orphans(dir.path()).unwrap();

        let pairs: Vec<(&str, &Path)> = modules
            .iter()
            .map(|m| (m.name.as_str(), m.rel_path.as_path()))
            .collect();

        assert_eq!(
            pairs,
            vec![
                ("greet", Path::new("greet.rs")),
                ("math", Path::new("math.rs")),
                ("nested::deep", Path::new("nested/deep.rs")),
                ("parent", Path::new("parent.rs")),
                ("utils", Path::new("utils/mod.rs")),
            ]
        );
    }

    #[test]
    fn skips_standalone_mod_rs() {
        let dir = tempfile::tempdir().unwrap();
        // A mod.rs at root level without a parent directory module —> should not appear
        fs::write(dir.path().join("mod.rs"), "").unwrap();
        fs::write(dir.path().join("real.rs"), "").unwrap();

        let modules = discover_orphans(dir.path()).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name, "real");
    }

    #[test]
    fn handles_dotted_filenames() {
        let dir = tempfile::tempdir().unwrap();
        // file.local.rs - extension is "rs", stem is "file.local"
        fs::write(dir.path().join("file.local.rs"), "").unwrap();
        fs::write(dir.path().join("simple.rs"), "").unwrap();

        let modules = discover_orphans(dir.path()).unwrap();

        let names: Vec<&str> = modules.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["file.local", "simple"]);
    }

    #[test]
    fn modern_layout_skips_submodule_dir() {
        let dir = tempfile::tempdir().unwrap();
        // file.rs alongside file/ directory with submodules.
        fs::write(dir.path().join("parent.rs"), "pub mod child;").unwrap();
        fs::create_dir_all(dir.path().join("parent")).unwrap();
        fs::write(dir.path().join("parent/child.rs"), "").unwrap();
        fs::write(dir.path().join("other.rs"), "").unwrap();

        let modules = discover_orphans(dir.path()).unwrap();

        let pairs: Vec<(&str, &Path)> = modules
            .iter()
            .map(|m| (m.name.as_str(), m.rel_path.as_path()))
            .collect();

        assert_eq!(
            pairs,
            vec![
                ("other", Path::new("other.rs")),
                ("parent", Path::new("parent.rs")),
            ]
        );
    }

    #[test]
    fn errors_on_file_and_mod_rs_conflict() {
        let dir = tempfile::tempdir().unwrap();
        // abc.rs and abc/mod.rs both claim module name "abc"
        fs::write(dir.path().join("abc.rs"), "").unwrap();
        fs::create_dir_all(dir.path().join("abc")).unwrap();
        fs::write(dir.path().join("abc/mod.rs"), "").unwrap();

        let err = discover_orphans(dir.path()).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("conflicting module 'abc'"),
            "unexpected error: {msg}"
        );
        assert!(msg.contains("abc.rs"), "should mention abc.rs: {msg}");
        assert!(
            msg.contains("abc/mod.rs"),
            "should mention abc/mod.rs: {msg}"
        );
    }
}
