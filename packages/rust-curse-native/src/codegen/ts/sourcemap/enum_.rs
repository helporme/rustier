//! Generates source map mappings for TypeScript enum declarations.

use rustdoc_types::ItemEnum;

use crate::codegen::{
    db::Db,
    ir::{enum_::Enum, shape::Shape},
    rustdoc::CrateExt,
    ts::{
        enum_::compile_enum,
        sourcemap::{
            Mapping,
            mappings::{find_name_line, original_location},
        },
    },
};

/// Compiles an IR enum and its emitted variants and fields into TypeScript
/// declaration mappings.
#[salsa::tracked(returns(ref))]
pub fn compile_enum_mappings<'db>(
    db: &'db dyn Db,
    enum_: Enum<'db>,
    generated_line: usize,
) -> Vec<Mapping> {
    let enum_rustdoc_id = enum_.id(db).rustdoc_id(db);
    let Some(enum_original) = original_location(db, enum_rustdoc_id) else {
        return Vec::new();
    };
    let enum_name = enum_.name(db).to_string();
    let generated = compile_enum(db, enum_);
    let enum_type_line = find_name_line(generated, 0, "export type ", &enum_name, " =")
        .expect("compiled TypeScript enum type declaration should exist");
    let namespace_line = find_name_line(
        generated,
        enum_type_line + 1,
        "export namespace ",
        &enum_name,
        " {",
    )
    .expect("compiled TypeScript enum namespace declaration should exist");
    let plain_line = find_name_line(
        generated,
        namespace_line + 1,
        "\texport type ",
        "Plain",
        " =",
    )
    .expect("compiled TypeScript enum plain type declaration should exist");
    let instance_line = find_name_line(
        generated,
        plain_line + 1,
        "\texport class ",
        "Instance",
        " {",
    )
    .expect("compiled TypeScript enum instance class declaration should exist");
    let constructor_line = find_name_line(
        generated,
        instance_line + 1,
        "\t\tprivate ",
        "constructor",
        "();",
    )
    .expect("compiled TypeScript enum instance constructor should exist");
    let enum_original_name_column = enum_original.column + "pub enum ".len();
    let mut mappings = Vec::new();

    for (line, column) in [
        (enum_type_line, "export type ".len()),
        (namespace_line, "export namespace ".len()),
        (plain_line, "\texport type ".len()),
        (instance_line, "\texport class ".len()),
        (constructor_line, "\t\tprivate ".len()),
    ] {
        let line = generated_line + line - enum_type_line;
        mappings.extend([
            Mapping {
                generated_line: line,
                generated_column: 0,
                source: enum_original.source.clone(),
                original_line: enum_original.line,
                original_column: enum_original.column,
                name: enum_name.clone(),
            },
            Mapping {
                generated_line: line,
                generated_column: column,
                source: enum_original.source.clone(),
                original_line: enum_original.line,
                original_column: enum_original_name_column,
                name: enum_name.clone(),
            },
        ]);
    }

    let enum_item = enum_rustdoc_id.item(db);
    let ItemEnum::Enum(enum_def) = &enum_item.inner else {
        unreachable!()
    };
    let crate_body = enum_rustdoc_id.crate_body(db);
    let tag_prefix = match enum_.js_tag_field_name(db) {
        "$type" => "\t\t\treadonly $type: \"",
        "type" => "\t\t\treadonly type: \"",
        _ => unreachable!(),
    };
    let mut variant_start_line = plain_line + 1;

    for ((variant, variant_id), tag_name) in enum_
        .variants(db)
        .iter()
        .zip(&enum_def.variants)
        .zip(enum_.js_tag_names(db))
    {
        let variant_id = crate_body.id(db, *variant_id);
        let tag_line = find_name_line(generated, variant_start_line, tag_prefix, tag_name, "\";")
            .expect("compiled TypeScript enum variant discriminant should exist");
        variant_start_line = tag_line + 1;

        if let Some(original) = original_location(db, variant_id) {
            let line = generated_line + tag_line - enum_type_line;
            let name = variant.name(db).clone();
            mappings.extend([
                Mapping {
                    generated_line: line,
                    generated_column: 0,
                    source: original.source.clone(),
                    original_line: original.line,
                    original_column: original.column,
                    name: name.clone(),
                },
                Mapping {
                    generated_line: line,
                    generated_column: "\t\t\treadonly ".len(),
                    source: original.source,
                    original_line: original.line,
                    original_column: original.column,
                    name,
                },
            ]);
        }

        for field in variant.fields(db) {
            let (prefix, suffix) = match (variant.shape(db), field.is_readonly(db)) {
                (Shape::Tuple(_), false) => ("\t\t\t[", "]:"),
                (Shape::Tuple(_), true) => ("\t\t\treadonly [", "]:"),
                (Shape::Plain { .. }, false) => ("\t\t\t", ":"),
                (Shape::Plain { .. }, true) => ("\t\t\treadonly ", ":"),
                (Shape::Unit, _) => unreachable!(),
            };
            let field_line = find_name_line(
                generated,
                variant_start_line,
                prefix,
                field.name(db),
                suffix,
            )
            .expect("compiled TypeScript enum variant field should exist");
            variant_start_line = field_line + 1;

            let Some(original) = original_location(db, field.rustdoc_id(db, enum_.id(db))) else {
                continue;
            };
            let line = generated_line + field_line - enum_type_line;
            let name = field.name(db).clone();
            mappings.extend([
                Mapping {
                    generated_line: line,
                    generated_column: 0,
                    source: original.source.clone(),
                    original_line: original.line,
                    original_column: original.column,
                    name: name.clone(),
                },
                Mapping {
                    generated_line: line,
                    generated_column: prefix.len(),
                    source: original.source,
                    original_line: original.line,
                    original_column: original.column,
                    name,
                },
            ]);
        }
    }

    mappings.sort_by_key(|mapping| (mapping.generated_line, mapping.generated_column));
    mappings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        db::DbImpl,
        rustdoc::enum_::lower_enum,
        test::{assert_snap, test_db},
        ts::sourcemap::test::snapshot_mappings,
    };

    fn compile_test_enum(db: DbImpl, path: &str, generated_line: usize) -> String {
        let rustdoc_id = db.rustdoc_id(path);
        let enum_ = lower_enum(&db, rustdoc_id).unwrap();
        let generated = compile_enum(&db, enum_);
        let mappings = compile_enum_mappings(&db, enum_, generated_line);

        snapshot_mappings(&db, rustdoc_id, generated, mappings)
    }

    #[test]
    fn documented_mixed_enum_with_acronym_variant() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                pub struct Local;

                /// A shape represented by several layouts.
                pub enum Shape {
                    /// An acronym-prefixed request.
                    HTTPRequest {
                        /// A field name repeated by the next variant.
                        response_code: i32,
                    },
                    /// A title-cased request with the same generated tag.
                    HttpRequest {
                        /// A repeated field name.
                        response_code: i32,
                    },
                    /// Coordinates represented by tuple fields.
                    Point(
                        /// A Rust-backed value that cannot be replaced.
                        Local,
                        i32,
                    ),
                    /// A variant without fields.
                    Unit,
                }
            };

            compile_test_enum(db, "Shape", 4)
        });
    }

    #[test]
    fn enum_with_type_field_uses_renamed_discriminant() {
        let db = test_db! {
            pub enum Example {
                Value { r#type: String },
            }
        };

        let compiled = compile_test_enum(db, "Example", 0);

        assert!(compiled.contains("readonly $type: \"value\";"));
    }

    #[test]
    fn empty_enum() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                pub enum Never {}
            };

            compile_test_enum(db, "Never", 0)
        });
    }
}
