//! Filesystem walk that returns orphan rust modules under a root directory.
//!
//! An orphan module is a standalone `.rs` file (or `folder/mod.rs`) that is
//! not declared as part of another module's `mod` tree.
//!
//! Examples:
//! - `src/greet.rs` -> orphan module `greet`
//! - `src/utils/mod.rs` -> orphan module `utils`
//! - `src/nested/deep.rs` -> orphan module `nested::deep`
//!
//! Directories with a `mod.rs` are treated as a single module (not recursed
//! into). Directories without `mod.rs` are recursed into to find deeper
//! orphans. A `file.rs` alongside a `file/` directory is the modern layout
//! and the directory's submodules are owned by `file.rs`, so the directory
//! is not recursed.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::OrphanScanError;

pub(super) fn walk_orphans(abs_root: &Path) -> Result<Vec<PathBuf>, OrphanScanError> {
    let mut rel_paths = Vec::new();
    walk_dir(abs_root, abs_root, &mut rel_paths)?;
    Ok(rel_paths)
}

fn walk_dir(
    abs_dir: &Path,
    abs_root: &Path,
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

    let rs_stems: HashSet<String> = rs_files
        .iter()
        .filter_map(|p| {
            let stem = p.file_stem()?.to_string_lossy().into_owned();
            (stem != "mod").then_some(stem)
        })
        .collect();

    for abs_path in &rs_files {
        let stem = abs_path.file_stem().unwrap().to_string_lossy();
        if stem == "mod" {
            continue;
        }
        let rel_path = abs_path.strip_prefix(abs_root).unwrap().to_path_buf();
        out.push(rel_path);
    }

    for abs_path in &dirs {
        let dir_name = abs_path.file_name().unwrap().to_string_lossy();
        let abs_mod_file = abs_path.join("mod.rs");

        if abs_mod_file.exists() {
            // Old layout: dir/mod.rs is itself an orphan module.
            let rel_path = abs_mod_file.strip_prefix(abs_root).unwrap().to_path_buf();
            out.push(rel_path);
        } else if rs_stems.contains(dir_name.as_ref()) {
            // Modern layout: file.rs alongside file/ directory. Submodules
            // inside file/ are owned by file.rs and are not orphans.
        } else {
            walk_dir(abs_path, abs_root, out)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_temp_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();

        fs::write(dir.path().join("greet.rs"), "pub fn hello() {}").unwrap();
        fs::write(dir.path().join("greet.js"), "const hello = 'kitty';").unwrap();

        fs::write(dir.path().join("math.rs"), "pub fn add() {}").unwrap();

        fs::create_dir_all(dir.path().join("utils")).unwrap();
        fs::write(dir.path().join("utils/mod.rs"), "pub fn help() {}").unwrap();
        fs::write(dir.path().join("utils/mod.js"), "const hello = 'kitty';").unwrap();

        fs::create_dir_all(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("nested/deep.rs"), "pub fn go() {}").unwrap();

        fs::write(dir.path().join("parent.rs"), "pub mod child;").unwrap();
        fs::create_dir_all(dir.path().join("parent")).unwrap();
        fs::write(dir.path().join("parent/child.rs"), "").unwrap();

        dir
    }

    fn sorted(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
        paths.sort();
        paths
    }

    #[test]
    fn discovers_orphan_modules() {
        let dir = setup_temp_dir();
        let abs_root = fs::canonicalize(dir.path()).unwrap();
        let paths = sorted(walk_orphans(&abs_root).unwrap());

        assert_eq!(
            paths,
            vec![
                PathBuf::from("greet.rs"),
                PathBuf::from("math.rs"),
                PathBuf::from("nested/deep.rs"),
                PathBuf::from("parent.rs"),
                PathBuf::from("utils/mod.rs"),
            ]
        );
    }

    #[test]
    fn skips_standalone_mod_rs() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("mod.rs"), "").unwrap();
        fs::write(dir.path().join("real.rs"), "").unwrap();

        let abs_root = fs::canonicalize(dir.path()).unwrap();
        let paths = sorted(walk_orphans(&abs_root).unwrap());

        assert_eq!(paths, vec![PathBuf::from("real.rs")]);
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
