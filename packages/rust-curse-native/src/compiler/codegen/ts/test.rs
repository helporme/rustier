//! Test-only helpers for validating generated TypeScript snapshots.

use std::path::Path;

use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::compiler::test::format_rust_source;

pub fn build_ts_snapshot(
    source: &str,
    compiled: &str,
    source_file: &str,
) -> (String, std::path::PathBuf) {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, compiled, SourceType::d_ts()).parse();

    assert!(
        parsed.diagnostics.is_empty(),
        "generated TypeScript must be syntactically valid:\n{:#?}\n\n{compiled}",
        parsed.diagnostics,
    );

    let source = format_rust_source(source);
    let compiled = compiled.trim_end();
    let snapshot =
        format!("# Source\n\n```rust\n{source}```\n\n# Compiled\n\n```ts\n{compiled}\n```");
    let snapshot_category = Path::new(source_file)
        .file_stem()
        .expect("snapshot source should have a file name");
    let snapshot_path = Path::new("_snapshots").join(snapshot_category);

    (snapshot, snapshot_path)
}

#[macro_export]
macro_rules! assert_ts_snap {
    (
        source: { $($source:tt)* },
        expression: |$db:ident, $printer:ident| $expression:block $(,)?
    ) => {{
        let mut __db = $crate::test_db! { $($source)* };
        $crate::compiler::inputs::Platform::set(
            &mut __db,
            $crate::compiler::inputs::Platform::Node,
        );
        $crate::compiler::inputs::FfiLayout::set(
            &mut __db,
            $crate::compiler::inputs::FfiLayout {
                ffi_js_file: $crate::compiler::FilePath::new_virtual("/ffi.js")
                    .expect("test FFI path must be valid"),
            },
        );
        let __compiled = $crate::compiler::codegen::fragment::FragmentPrint::print(
            $crate::compiler::codegen::fragment::fragment(|$printer| {
                let $db = &mut __db;
                $expression
            }),
        );

        let (snapshot, snapshot_path) =
            $crate::compiler::codegen::ts::test::build_ts_snapshot(
                stringify!($($source)*),
                &__compiled,
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
