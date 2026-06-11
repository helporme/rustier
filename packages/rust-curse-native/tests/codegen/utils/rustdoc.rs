//! Utilities for generating rustdoc JSON from inline test fixtures.

use std::{collections::BTreeMap, fs, path::Path, process::Command};

use rust_curse::codegen::{
    db::{Db, DbImpl},
    rustdoc::{RustdocId, RustdocInput},
};
use rustdoc_types::Crate as RustdocCrate;
use tempfile::TempDir;

pub const TEST_CRATE_NAME: &str = "rustdoc_test_crate";

pub struct Fixture {
    pub _dir: TempDir,
    pub root_abs_path: std::path::PathBuf,
    pub src: String,
    pub crate_: RustdocCrate,
}

pub fn rustdoc_from_src(src: &str) -> Fixture {
    let src = fmt_src(src);
    let dir = tempfile::tempdir().expect("create tempdir");
    let root_abs_path = dir.path().to_path_buf();

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

    fs::write(root_abs_path.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
    fs::create_dir_all(root_abs_path.join("src")).expect("create src/");
    fs::write(root_abs_path.join("src/lib.rs"), &src).expect("write src/lib.rs");

    let target_abs_path = root_abs_path.join("target");
    let crate_ = run_rustdoc(&root_abs_path, &target_abs_path, TEST_CRATE_NAME)
        .unwrap_or_else(|err| panic!("rustdoc generation failed: {err}"));

    Fixture {
        _dir: dir,
        root_abs_path,
        src,
        crate_,
    }
}

pub fn fmt_src(src: &str) -> String {
    match syn::parse_str::<syn::File>(src) {
        Ok(file) => prettyplease::unparse(&file),
        Err(_) => src.to_string(),
    }
}

fn run_rustdoc(
    project_abs_path: &Path,
    target_abs_path: &Path,
    lib_name: &str,
) -> Result<RustdocCrate, String> {
    let output = Command::new("cargo")
        .args([
            "+nightly",
            "rustdoc",
            "--lib",
            "--",
            "-Z",
            "unstable-options",
            "--output-format",
            "json",
        ])
        .env("RUST_CURSE_RUSTDOC", "1")
        .env("CARGO_TARGET_DIR", target_abs_path)
        .current_dir(project_abs_path)
        .output()
        .map_err(|err| format!("cargo is not available: {err}"))?;

    if !output.status.success() {
        return Err(format!(
            "cargo rustdoc failed (exit code {:?}):\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let json_abs_path = target_abs_path.join("doc").join(format!("{lib_name}.json"));
    let json = fs::read_to_string(&json_abs_path)
        .map_err(|err| format!("failed to read '{}': {err}", json_abs_path.display()))?;

    serde_json::from_str(&json).map_err(|err| format!("failed to parse rustdoc JSON: {err}"))
}

pub fn db_with_rustdoc(crate_: RustdocCrate) -> DbImpl {
    let db = DbImpl::default();
    let crates = BTreeMap::from([(TEST_CRATE_NAME.to_string(), crate_)]);
    RustdocInput::new(&db, crates);
    db
}

pub fn root_id<'db>(db: &'db dyn Db, crate_: &RustdocCrate) -> RustdocId<'db> {
    RustdocId::new(db, crate_.root, TEST_CRATE_NAME)
}
