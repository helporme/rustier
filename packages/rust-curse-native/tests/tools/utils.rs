use std::{fs, path::PathBuf};

use rustdoc_types::Crate as RustdocCrate;
use tempfile::TempDir;

pub struct Fixture {
    pub _dir: TempDir,
    pub root_abs_path: PathBuf,
}

pub fn fixture(files: &[(&str, &str)]) -> Fixture {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root_abs_path = dir.path().to_path_buf();

    for (rel_path, contents) in files {
        let abs_path = root_abs_path.join(rel_path);
        if let Some(parent_abs_path) = abs_path.parent() {
            fs::create_dir_all(parent_abs_path).expect("create fixture dirs");
        }
        fs::write(&abs_path, contents).expect("write fixture file");
    }

    Fixture {
        _dir: dir,
        root_abs_path,
    }
}

pub fn has_item_named(crate_: &RustdocCrate, item_name: &str) -> bool {
    crate_
        .index
        .values()
        .any(|item| item.name.as_deref() == Some(item_name))
}
