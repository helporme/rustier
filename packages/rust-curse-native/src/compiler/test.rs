//! Shared compiler test fixtures.

#![allow(dead_code)]

use std::collections::BTreeMap;

use crate::compiler::{
    db::DbImpl,
    inputs::{Crate, RustdocCrates},
};

/// Name of the temporary source crate created by [`test_db!`].
#[doc(hidden)]
pub const TEST_CRATE_NAME: &str = "rustdoc_test_crate";

/// Formats tokenized Rust source using standard Rust indentation.
#[doc(hidden)]
pub fn format_rust_source(source: &str) -> String {
    syn::parse_file(source)
        .map(|file| prettyplease::unparse(&file))
        .unwrap_or_else(|_| source.to_owned())
}

/// Builds a code generation database from root source and additional module
/// files.
#[doc(hidden)]
pub fn build_test_db(source: String, files: &[(&str, String)]) -> DbImpl {
    let fixture_dir = tempfile::tempdir().expect("create test crate tempdir");
    let crate_dir = fixture_dir.path().to_path_buf();
    let src_dir = crate_dir.join("src");

    std::fs::create_dir_all(&src_dir).expect("create test crate src directory");
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        indoc::formatdoc! {r#"
            [package]
            name = "{TEST_CRATE_NAME}"
            version = "0.0.0"
            edition = "2021"
            publish = false

            [lib]
            path = "src/lib.rs"

            [workspace]
        "#},
    )
    .expect("write test crate manifest");
    std::fs::write(src_dir.join("lib.rs"), source).expect("write test crate source");

    for (path, source) in files {
        let file_path = src_dir.join(path);
        std::fs::create_dir_all(
            file_path
                .parent()
                .expect("test source file should have a parent directory"),
        )
        .expect("create test source file directory");
        std::fs::write(file_path, source).expect("write test source file");
    }

    let crate_body = crate::tools::rustdoc::run(&crate_dir.join("Cargo.toml"))
        .unwrap_or_else(|error| panic!("rustdoc generation failed: {error}"));
    let crates = BTreeMap::from([(
        TEST_CRATE_NAME.to_owned(),
        Crate {
            body: crate_body,
            is_external: false,
            dir: crate_dir,
        },
    )]);

    let mut db = DbImpl::default();
    RustdocCrates::set(&mut db, crates);

    // Source files are read lazily by diagnostics and sourcemap queries, so
    // they must outlive the returned database.
    let _ = fixture_dir.keep();
    db
}

/// Creates a [`DbImpl`] containing rustdoc inputs for inline Rust code.
#[macro_export]
macro_rules! test_db {
    (
        @parse;
        @source: [$($source:tt)*];
        @bodies: [$($bodies:tt)*];
        @file: $path:literal;
        $visibility:vis mod $name:ident { $($body:tt)* }
        $($rest:tt)*
    ) => {
        $crate::test_db! {
            @parse;
            @source: [
                $($source)*
                #[path = $path]
                $visibility mod $name;
            ];
            @bodies: [
                $($bodies)*
                ($path, { $($body)* })
            ];
            $($rest)*
        }
    };
    (
        @parse;
        @source: [$($source:tt)*];
        @bodies: [$($bodies:tt)*];
        $token:tt
        $($rest:tt)*
    ) => {
        $crate::test_db! {
            @parse;
            @source: [$($source)* $token];
            @bodies: [$($bodies)*];
            $($rest)*
        }
    };
    (
        @parse;
        @source: [$($source:tt)*];
        @bodies: [$(($path:literal, { $($body:tt)* }))*];
    ) => {{
        let source = $crate::compiler::test::format_rust_source(
            ::std::stringify!($($source)*),
        );
        let files = [$(
            (
                $path,
                $crate::compiler::test::format_rust_source(
                    ::std::stringify!($($body)*),
                ),
            )
        ),*];
        $crate::compiler::test::build_test_db(source, &files)
    }};
    ($($source:tt)*) => {
        $crate::test_db! {
            @parse;
            @source: [];
            @bodies: [];
            $($source)*
        }
    };
}
