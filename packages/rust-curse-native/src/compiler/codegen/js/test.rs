//! Test-only helpers for validating generated JavaScript snapshots.

use std::path::Path;

use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::compiler::test::format_rust_source;

/// Builds a snapshot containing formatted Rust source and generated JavaScript.
pub fn build_js_snapshot(
    source: &str,
    compiled: &str,
    source_file: &str,
) -> (String, std::path::PathBuf) {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, compiled, SourceType::mjs()).parse();

    assert!(
        parsed.diagnostics.is_empty(),
        "generated JavaScript must be syntactically valid:\n{:#?}\n\n{compiled}",
        parsed.diagnostics,
    );

    let source = format_rust_source(source);
    let compiled = compiled.trim_end();
    let snapshot =
        format!("# Source\n\n```rust\n{source}```\n\n# Compiled\n\n```js\n{compiled}\n```");
    let snapshot_category = Path::new(source_file)
        .file_stem()
        .expect("snapshot source should have a file name");
    let snapshot_path = Path::new("_snapshots").join(snapshot_category);

    (snapshot, snapshot_path)
}

/// Asserts formatted source and syntactically valid generated JavaScript.
#[macro_export]
macro_rules! assert_js_snap {
    (
        source: { $($source:tt)* },
        expression: |$db:ident, $printer:ident| $expression:block $(,)?
    ) => {{
        let mut __db = $crate::test_db! { $($source)* };
        $crate::compiler::inputs::Api::set(
            &mut __db,
            $crate::compiler::inputs::Api::Node,
        );
        let __compiled = $crate::compiler::codegen::fragment::FragmentPrint::print(
            $crate::compiler::codegen::fragment::fragment(|$printer| {
                let $db = &mut __db;
                $expression
            }),
        );

        let (snapshot, snapshot_path) =
            $crate::compiler::codegen::js::test::build_js_snapshot(
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
