//! Utilities for generating rustdoc JSON from inline test fixtures.

use std::{collections::BTreeMap, fs, path::PathBuf};

use rust_curse::tools::rustdoc::{Crate as StoredCrate, RunOpts};
use rust_curse::{
    codegen::{
        db::{Db, DbImpl},
        rustdoc::{Id, Input},
    },
    tools::rustdoc,
};
use rustdoc_types::Crate as RustdocCrate;
use tempfile::TempDir;

pub const TEST_CRATE_NAME: &str = "rustdoc_test_crate";

pub struct Fixture {
    pub _dir: TempDir,
    pub src: String,
    pub crate_: RustdocCrate,
    pub crate_dir: PathBuf,
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

    let crate_ = rustdoc::run(RunOpts {
        manifest_path: root_abs_path.join("Cargo.toml"),
        crate_name: None,
    })
    .unwrap_or_else(|err| panic!("rustdoc generation failed: {err}"));

    Fixture {
        _dir: dir,
        src,
        crate_dir: crate_.dir,
        crate_: crate_.body,
    }
}

pub fn fmt_src(src: &str) -> String {
    match syn::parse_str::<syn::File>(src) {
        Ok(file) => prettyplease::unparse(&file),
        Err(_) => src.to_string(),
    }
}

pub fn db_with_rustdoc(crate_: RustdocCrate, dir: impl Into<PathBuf>) -> DbImpl {
    let db = DbImpl::default();
    let crates = BTreeMap::from([(
        TEST_CRATE_NAME.to_string(),
        StoredCrate {
            body: crate_,
            is_external: false,
            dir: dir.into(),
        },
    )]);
    Input::new(&db, crates);
    db
}

pub fn root_id<'db>(db: &'db dyn Db, crate_: &RustdocCrate) -> Id<'db> {
    Id::new(db, crate_.root, TEST_CRATE_NAME)
}
