//! Generates source map mappings for TypeScript function declarations.

use crate::codegen::{
    db::Db,
    ir::{fn_::Fn, item::ItemIdExt},
    ts::sourcemap::{Mapping, mappings::original_location},
};

/// Compiles an IR function into mappings for its TypeScript declaration line.
#[salsa::tracked(returns(ref))]
pub fn compile_fn_mappings<'db>(
    db: &'db dyn Db,
    fn_: Fn<'db>,
    generated_line: usize,
) -> Vec<Mapping> {
    let Some(span) = fn_.span(db) else {
        return Vec::new();
    };
    let original = original_location(span);
    let name = fn_.name(db).to_string();

    vec![
        Mapping {
            generated_line,
            generated_column: 0,
            source: original.source.clone(),
            original_line: original.line,
            original_column: original.column,
            name: name.clone(),
        },
        Mapping {
            generated_line,
            generated_column: "export function ".len(),
            source: original.source,
            original_line: original.line,
            original_column: original.column + "pub fn ".len(),
            name,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        db::DbImpl,
        rustdoc::mod_::lower_mods,
        test::{TEST_CRATE_NAME, assert_snap, test_db},
        ts::{fn_::compile_fn, sourcemap::test::snapshot_mappings},
    };

    fn compile_test_fn(db: DbImpl, path: &str, generated_line: usize) -> String {
        let rustdoc_id = db.rustdoc_id(path);
        let fqn = format!("{TEST_CRATE_NAME}::{path}");
        let modules = lower_mods(&db);
        let fn_ = modules
            .iter()
            .flat_map(|mod_| mod_.fns(&db))
            .find(|fn_| fn_.fqn(&db) == &fqn)
            .expect("test function should be lowered");
        let generated = compile_fn(&db, fn_);
        let mappings = compile_fn_mappings(&db, fn_, generated_line);

        snapshot_mappings(&db, rustdoc_id, generated, mappings)
    }

    #[test]
    fn fn_at_generated_start() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                pub fn foo() {}
            };

            compile_test_fn(db, "foo", 0)
        });
    }

    #[test]
    fn fn_after_generated_lines() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                pub fn foo(value: i32) -> i32 {
                    value
                }
            };

            compile_test_fn(db, "foo", 7)
        });
    }

    #[test]
    fn fn_nested_in_inline_mod() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                pub mod nested {
                    pub fn foo(value: i32) -> i32 {
                        value
                    }
                }
            };

            compile_test_fn(db, "nested::foo", 0)
        });
    }

    #[test]
    fn fn_in_external_file() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                @file: "nested/functions.rs";
                pub mod functions {
                    pub fn foo() {}
                }
            };

            compile_test_fn(db, "functions::foo", 0)
        });
    }
}
