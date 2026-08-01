//! Test-only helpers for validating generated Rust bindings snapshots.

use std::{path::Path, process::Command};

use crate::codegen::{
    api::Api, db::Db, fragment::FragmentPrint as _, rustdoc::RustdocCrates, test::TEST_CRATE_NAME,
};

fn format_rust(source: &str, description: &str) -> String {
    let syntax = syn::parse_file(source).unwrap_or_else(|error| {
        panic!("{description} must be valid Rust syntax: {error}\n\n{source}")
    });
    prettyplease::unparse(&syntax)
}

fn cargo_check(db: &dyn Db, compiled: &str) {
    let rustdoc = RustdocCrates::get(db);
    let source_crate = rustdoc
        .crate_data(db, TEST_CRATE_NAME)
        .expect("bindings snapshots require a test_db! source crate");
    let source_path = source_crate
        .dir
        .canonicalize()
        .expect("canonicalize bindings snapshot source crate path");
    let source_path = source_path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let api_dependencies = Api::get(db).manifest_deps(db).print();

    let fixture = tempfile::tempdir().expect("create generated bindings check crate");
    let fixture_src = fixture.path().join("src");
    std::fs::create_dir(&fixture_src).expect("create generated bindings check crate src directory");
    std::fs::write(
        fixture.path().join("Cargo.toml"),
        format!(
            r#"[package]
name = "rust-curse-bindings-snapshot"
version = "0.0.0"
edition = "2021"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
{TEST_CRATE_NAME} = {{ path = "{source_path}" }}
{api_dependencies}
[workspace]
"#,
        ),
    )
    .expect("write generated bindings check crate manifest");
    std::fs::write(fixture_src.join("lib.rs"), compiled)
        .expect("write generated bindings check crate source");

    let target_dir = std::env::temp_dir().join("rust-curse-bindings-snapshot-target");
    let output = Command::new("cargo")
        .args(["check", "--quiet", "--color", "never"])
        .current_dir(fixture.path())
        .env("CARGO_TARGET_DIR", target_dir)
        .output()
        .expect("run cargo check on generated bindings");

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "generated bindings failed cargo check\n\nstdout:\n{stdout}\n\nstderr:\n{stderr}\n\nsource:\n{compiled}"
        );
    }
}

/// Builds a snapshot containing formatted Rust source and bindings for both
/// APIs.
#[doc(hidden)]
pub fn build_rust_snapshot(
    node_db: &dyn Db,
    wasm_db: &dyn Db,
    source: &str,
    node: &str,
    wasm: &str,
    source_file: &str,
    check: bool,
) -> (String, std::path::PathBuf) {
    let source = format_rust(source, "snapshot source");
    let node = format_rust(node, "compiled Node bindings");
    let wasm = format_rust(wasm, "compiled Wasm bindings");

    if check {
        cargo_check(node_db, &node);
        cargo_check(wasm_db, &wasm);
    }

    let snapshot = format!(
        "# Source\n\n```rust\n{source}```\n\n# Compiled\n## Node\n```rust\n{node}```\n\n## Wasm\n```rust\n{wasm}```"
    );
    let snapshot_category = Path::new(source_file)
        .file_stem()
        .expect("snapshot source should have a file name");
    let snapshot_path = Path::new("_snapshots").join(snapshot_category);

    (snapshot, snapshot_path)
}

/// Asserts formatted source and compiled Rust bindings in one snapshot.
#[macro_export]
macro_rules! assert_rust_snap {
    (
        cargo_check: $check:expr,
        source: { $($source:tt)* },
        expression: |$db:ident, $printer:ident| $expression:block $(,)?
    ) => {{
        let mut __node_db = $crate::test_db! { $($source)* };
        $crate::codegen::api::Api::set(
            &mut __node_db,
            $crate::codegen::api::ApiKind::Node,
        );
        let __node = $crate::codegen::fragment::FragmentPrint::print(
            $crate::codegen::fragment::fragment(|$printer| {
                let $db = &mut __node_db;
                $expression
            }),
        );

        let mut __wasm_db = $crate::test_db! { $($source)* };
        $crate::codegen::api::Api::set(
            &mut __wasm_db,
            $crate::codegen::api::ApiKind::Wasm {
                target: $crate::codegen::api::WasmTarget::Web,
            },
        );
        let __wasm = $crate::codegen::fragment::FragmentPrint::print(
            $crate::codegen::fragment::fragment(|$printer| {
                let $db = &mut __wasm_db;
                $expression
            }),
        );

        let (snapshot, snapshot_path) =
            $crate::codegen::bindings::test::build_rust_snapshot(
                &__node_db,
                &__wasm_db,
                stringify!($($source)*),
                &__node,
                &__wasm,
                file!(),
                $check,
            );

        ::insta::with_settings!({
            snapshot_path => snapshot_path,
            prepend_module_to_snapshot => false,
        }, {
            ::insta::assert_snapshot!(snapshot);
        });
    }};
    (
        source: { $($source:tt)* },
        expression: |$db:ident, $printer:ident| $expression:block $(,)?
    ) => {
        $crate::assert_rust_snap! {
            cargo_check: false,
            source: { $($source)* },
            expression: |$db, $printer| $expression,
        }
    };
}

pub use crate::assert_rust_snap;
