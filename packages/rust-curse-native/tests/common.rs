//! Shared test helpers. Declared from each integration test via
//! `#[path = "common.rs"] mod common;` (or plain `mod common;` — cargo's
//! `autotests = false` keeps this file from being compiled as its own test
//! binary).
//!
//! Runs real `cargo +nightly rustdoc` on a string of source code and returns
//! the parsed `rustdoc_types::Crate`. Use via:
//!
//! ```ignore
//! let crate_ = rustdoc! {
//!     pub fn add(a: i32, b: i32) -> i32 { a + b }
//! };
//! let allowed = allowed_by_name(&crate_, &["add"]);
//! ```

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rust_curse::discovery::rustdoc::run_cargo_rustdoc_json;

const TEST_CRATE_NAME: &str = "rustdoc_test_crate";

/// Shared target directory for all test rustdoc invocations. Lets cargo reuse
/// its incremental cache across tests -> first test pays the cost, reruns are
/// fast. Scoped under the system temp dir; persists across runs on most OSes.
fn shared_target_abs_dir() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| std::env::temp_dir().join("rust_curse_rustdoc_test_cache"))
}

pub fn gen_rustdoc(src: &str) -> rustdoc_types::Crate {
    let project_dir = tempfile::tempdir().expect("create tempdir");
    let project_abs_path = project_dir.path();

    let cargo_toml = format!(
        "[package]\n\
         name = \"{TEST_CRATE_NAME}\"\n\
         version = \"0.0.0\"\n\
         edition = \"2021\"\n\
         publish = false\n\
         \n\
         [lib]\n\
         path = \"src/lib.rs\"\n\
         \n\
         [workspace]\n"
    );
    fs::write(project_abs_path.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
    fs::create_dir_all(project_abs_path.join("src")).expect("create src/");
    fs::write(project_abs_path.join("src/lib.rs"), src).expect("write src/lib.rs");

    let target_abs_dir = shared_target_abs_dir();
    fs::create_dir_all(target_abs_dir).expect("create shared target dir");

    run_cargo_rustdoc_json(project_abs_path, target_abs_dir, TEST_CRATE_NAME)
        .unwrap_or_else(|e| panic!("rustdoc generation failed: {e}"))
}

/// Resolves item names to rustdoc Ids. Panics on missing or ambiguous names.
pub fn allowed_by_name(
    crate_: &rustdoc_types::Crate,
    names: &[&str],
) -> HashSet<rustdoc_types::Id> {
    let mut out = HashSet::new();
    for name in names {
        let matches: Vec<_> = crate_
            .index
            .iter()
            .filter(|(_, it)| it.name.as_deref() == Some(*name))
            .map(|(id, _)| *id)
            .collect();
        match matches.as_slice() {
            [] => panic!("no item named `{name}` in crate"),
            [id] => {
                out.insert(*id);
            }
            many => panic!("name `{name}` ambiguous: {} matches", many.len()),
        }
    }
    out
}

#[allow(dead_code)]
pub fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let src_abs_path = entry.path();
        let dst_abs_path = dst.join(entry.file_name());
        if src_abs_path.is_dir() {
            if entry.file_name() == "target" {
                continue;
            }
            copy_dir_recursive(&src_abs_path, &dst_abs_path);
        } else {
            fs::copy(&src_abs_path, &dst_abs_path).unwrap();
        }
    }
}

#[macro_export]
macro_rules! rustdoc {
    ($($tt:tt)*) => {
        $crate::common::gen_rustdoc(stringify!($($tt)*))
    };
}
