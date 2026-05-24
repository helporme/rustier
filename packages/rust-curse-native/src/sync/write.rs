use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Writes `bytes` to `abs_path` only when the file does not already contain
/// the exact same bytes. Returns `true` when a write actually happened.
///
/// The skip-if-unchanged path preserves the file's mtime, which avoids
/// triggering rust-analyzer reindexing, TS server reloads, and cargo
/// rebuild invalidation when a sync run produces the same output as last
/// time. The caller is responsible for ensuring `abs_path`'s parent
/// directory exists.
pub fn write_if_changed(abs_path: &Path, bytes: &[u8]) -> io::Result<bool> {
    if let Ok(existing) = fs::read(abs_path) {
        if existing == bytes {
            return Ok(false);
        }
    }
    fs::write(abs_path, bytes)?;
    Ok(true)
}

/// Makes `abs_dir` contain exactly the files described by `targets` and
/// nothing else, then removes `abs_dir` itself if it ends up empty.
///
/// Each `targets[i].0` is a path relative to `abs_dir`. Files that match
/// a target are written via [`write_if_changed`] so unchanged content
/// keeps its mtime; files under `abs_dir` that are not in `targets` are
/// deleted, and any empty subdirectories left behind are pruned
/// bottom-up. Used in place of `remove_dir_all` + bulk rewrite to avoid
/// briefly deleting files that editors have open.
pub fn sync_dir(abs_dir: &Path, targets: &[(PathBuf, Vec<u8>)]) -> io::Result<()> {
    let target_set: HashSet<&Path> = targets.iter().map(|(rel, _)| rel.as_path()).collect();

    if abs_dir.exists() {
        let mut existing: Vec<PathBuf> = Vec::new();
        collect_files(abs_dir, abs_dir, &mut existing)?;
        for rel in &existing {
            if !target_set.contains(rel.as_path()) {
                fs::remove_file(abs_dir.join(rel))?;
            }
        }
    }

    for (rel, content) in targets {
        let abs_path = abs_dir.join(rel);
        if let Some(parent) = abs_path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_if_changed(&abs_path, content)?;
    }

    if abs_dir.exists() {
        prune_empty_subdirs(abs_dir)?;
        if fs::read_dir(abs_dir)?.next().is_none() {
            fs::remove_dir(abs_dir)?;
        }
    }

    Ok(())
}

fn collect_files(abs_root: &Path, abs_dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(abs_dir)? {
        let entry = entry?;
        let abs_path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(abs_root, &abs_path, out)?;
        } else {
            let rel = abs_path
                .strip_prefix(abs_root)
                .expect("path under abs_root")
                .to_path_buf();
            out.push(rel);
        }
    }
    Ok(())
}

fn prune_empty_subdirs(abs_dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(abs_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let abs_sub = entry.path();
        prune_empty_subdirs(&abs_sub)?;
        if fs::read_dir(&abs_sub)?.next().is_none() {
            fs::remove_dir(&abs_sub)?;
        }
    }
    Ok(())
}
