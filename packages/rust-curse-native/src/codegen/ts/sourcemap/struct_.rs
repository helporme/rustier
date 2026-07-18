//! Generates source map mappings for TypeScript class declarations.

use crate::codegen::{
    db::Db,
    ir::struct_::{Struct, StructShape},
    ts::{
        sourcemap::{
            Mapping,
            mappings::{find_name_line, original_location},
        },
        struct_::compile_struct,
    },
};

/// Compiles an IR struct and its emitted fields into TypeScript declaration
/// mappings.
#[salsa::tracked(returns(ref))]
pub fn compile_struct_mappings<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
    generated_line: usize,
) -> Vec<Mapping> {
    let struct_rustdoc_id = struct_.id(db).rustdoc_id(db);
    let Some(original) = original_location(db, struct_rustdoc_id) else {
        return Vec::new();
    };
    let name = struct_.name(db).to_string();
    let generated = compile_struct(db, struct_);
    let class_line = find_name_line(generated, 0, "export class ", &name, " {")
        .expect("compiled TypeScript class declaration should exist");
    let mut mappings = vec![
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
            generated_column: "export class ".len(),
            source: original.source,
            original_line: original.line,
            original_column: original.column + "pub struct ".len(),
            name,
        },
    ];

    let is_plain = matches!(struct_.shape(db), StructShape::Plain { .. });
    let mut field_start_line = class_line + 1;
    for field in struct_.fields(db) {
        let prefix = if field.is_readonly(db) {
            "\tpublic readonly "
        } else {
            "\tpublic "
        };
        let field_line = find_name_line(generated, field_start_line, prefix, field.name(db), ":")
            .expect("compiled TypeScript class field declaration should exist");
        field_start_line = field_line + 1;

        let Some(original) = original_location(db, field.rustdoc_id(db, struct_.id(db))) else {
            continue;
        };

        let generated_line = generated_line + field_line - class_line;
        let name = field.name(db).clone();
        let original_name_column = if is_plain {
            original.column + "pub ".len()
        } else {
            original.column
        };
        mappings.extend([
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
                generated_column: prefix.len(),
                source: original.source,
                original_line: original.line,
                original_column: original_name_column,
                name,
            },
        ]);
    }

    mappings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        db::DbImpl,
        rustdoc::struct_::lower_struct,
        test::{assert_snap, test_db},
        ts::sourcemap::test::snapshot_mappings,
    };

    fn compile_test_struct(db: DbImpl, path: &str, generated_line: usize) -> String {
        let rustdoc_id = db.rustdoc_id(path);
        let struct_ = lower_struct(&db, rustdoc_id).unwrap();
        let generated = compile_struct(&db, struct_);
        let mappings = compile_struct_mappings(&db, struct_, generated_line);

        snapshot_mappings(&db, rustdoc_id, generated, mappings)
    }

    #[test]
    fn unit_struct() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                pub struct Foo;
            };

            compile_test_struct(db, "Foo", 0)
        });
    }

    #[test]
    fn documented_plain_struct() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                pub struct Local;

                /// Stores primitive and local values.
                pub struct Foo {
                    /// A numeric value.
                    pub number: i32,
                    /// A value that cannot be replaced from JavaScript.
                    pub local: Local,
                }
            };

            compile_test_struct(db, "Foo", 1)
        });
    }

    #[test]
    fn tuple_struct_with_private_field() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                pub struct Foo(pub i32, String, pub String);
            };

            compile_test_struct(db, "Foo", 0)
        });
    }

    #[test]
    fn struct_in_external_file_after_generated_lines() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                @file: "nested/models.rs";
                pub mod models {
                    pub struct Foo {
                        pub value: i32,
                    }
                }
            };

            compile_test_struct(db, "models::Foo", 5)
        });
    }
}
