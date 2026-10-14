//! Test-only helpers for validating generated Rust bindings snapshots.

use std::path::Path;

fn format_rust(source: &str, description: &str) -> String {
    let syntax = syn::parse_file(source).unwrap_or_else(|error| {
        panic!("{description} must be valid Rust syntax: {error}\n\n{source}")
    });
    prettyplease::unparse(&syntax)
}

/// Builds a snapshot containing formatted Rust source and bindings for both
/// APIs.
#[doc(hidden)]
pub fn build_rust_snapshot(
    source: &str,
    node: &str,
    wasm: &str,
    source_file: &str,
) -> (String, std::path::PathBuf) {
    let source = format_rust(source, "snapshot source");
    let node = format_rust(node, "compiled Node bindings");
    let wasm = format_rust(wasm, "compiled Wasm bindings");

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
        source: { $($source:tt)* },
        expression: |$db:ident, $transport:ident, $printer:ident| $expression:block $(,)?
    ) => {{
        let __db = $crate::test_db! { $($source)* };
        let (__node, __wasm) = ::ra_ap_hir::attach_db(&__db, || {
            let __node = $crate::compiler::codegen::fragment::FragmentPrint::print(
                $crate::compiler::codegen::fragment::fragment(|$printer| {
                    let $db = &__db;
                    let $transport = $crate::compiler::ir::abi::transport::AbiTransport::Napi;
                    $expression
                }),
            );

            let __wasm = $crate::compiler::codegen::fragment::FragmentPrint::print(
                $crate::compiler::codegen::fragment::fragment(|$printer| {
                    let $db = &__db;
                    let $transport = $crate::compiler::ir::abi::transport::AbiTransport::Wasm(
                        $crate::compiler::ir::abi::transport::WasmTransport {
                            memory: $crate::compiler::ir::abi::transport::WasmMemory::x32,
                        },
                    );
                    $expression
                }),
            );

            (__node, __wasm)
        });

        let (snapshot, snapshot_path) =
            $crate::compiler::codegen::runtime::test::build_rust_snapshot(
                stringify!($($source)*),
                &__node,
                &__wasm,
                file!(),
            );

        ::insta::with_settings!({
            snapshot_path => snapshot_path,
            prepend_module_to_snapshot => false,
        }, {
            ::insta::assert_snapshot!(snapshot);
        });
    }};
}
