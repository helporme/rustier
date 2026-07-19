use std::collections::BTreeSet;

use crate::{
    codegen::{
        db::Db,
        fragment::{FragmentOnce, FragmentPrint, blank, curlies, doubles, fragment, idented},
        ir::{
            enum_::{Enum, EnumVariant, enum_name_from_ir_id},
            shape::Shape,
            struct_::struct_name_from_ir_id,
            type_::Type,
        },
        js::imports::{Import, ImportFrom},
        ts::jsdoc::compile_jsdoc,
    },
    frag,
};

/// Compiles an IR enum into a discriminated TypeScript union backed by a
/// nominal instance class.
///
/// Requires imports added by [`compile_enum_required_imports`] to be in scope.
#[salsa::tracked(returns(ref))]
pub fn compile_enum<'db>(db: &'db dyn Db, enum_: Enum<'db>) -> String {
    let variants = fragment(move |printer| {
        for (index, variant) in enum_.variants(db).iter().enumerate() {
            if index > 0 {
                blank().apply(printer);
            }
            compile_variant(db, *variant).apply(printer);
        }
        ";".apply(printer);
    });
    let plain = fragment(move |printer| {
        "export type Plain =".apply(printer);

        if enum_.variants(db).is_empty() {
            " never;".apply(printer);
        } else {
            blank().apply(printer);
            idented(variants).apply(printer);
        }
    });

    frag!(
        {compile_jsdoc(enum_.docs(db).as_deref())}
        export type ~{enum_.name(db)}~ = ~{enum_.name(db)}.Instance & ~{enum_.name(db)}.Plain;@@

        export namespace ~{enum_.name(db)}~{{
            {plain}@@

            export class Instance~{{
                private __rust_curse_brand: never;@
                private constructor();
            }}
        }}
    )
    .print()
}

/// Adds imports required by [`compile_enum`].
pub fn compile_enum_required_imports<'db>(
    db: &'db dyn Db,
    enum_: Enum<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    let active_mod_id = enum_.id(db).get_parent_mod_id(db).unwrap();

    for variant in enum_.variants(db) {
        for field in variant.fields(db) {
            let (type_id, type_name) = match field.type_(db) {
                Type::Struct(struct_id) => (struct_id, struct_name_from_ir_id(db, struct_id)),
                Type::Enum(enum_id) => (enum_id, enum_name_from_ir_id(db, enum_id)),
                Type::Primitive(_) | Type::Str | Type::String => continue,
            };
            let type_mod_id = type_id.get_parent_mod_id(db).unwrap();

            if active_mod_id != type_mod_id {
                imports.insert(Import {
                    item: type_name,
                    from: ImportFrom::RelativeToMod {
                        from: active_mod_id,
                        to: type_mod_id,
                    },
                });
            }
        }
    }
}

fn compile_variant<'db>(db: &'db dyn Db, variant: EnumVariant<'db>) -> impl FragmentOnce + 'db {
    let fields = variant.fields(db);
    let properties = fragment(move |printer| {
        frag!(readonly type:~{doubles(variant.js_tag_name(db))};).apply(printer);

        for field in fields.clone() {
            blank().apply(printer);
            compile_jsdoc(field.docs(db).as_deref()).apply(printer);

            let readonly = if field.is_readonly(db) {
                "readonly "
            } else {
                ""
            };
            let field_name = fragment(move |printer| match variant.shape(db) {
                Shape::Tuple(_) => frag!([{ field.name(db) }]).apply(printer),
                Shape::Plain { .. } => field.name(db).apply(printer),
                Shape::Unit => unreachable!(),
            });

            frag!({readonly}{field_name}:~{field.type_(db).ts_name(db)};).apply(printer);
        }
    });

    frag!(
        {compile_jsdoc(variant.docs(db).as_deref())}
        |~{curlies(properties)}
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        db::DbImpl,
        fragment::{FragmentPrint, spaced},
        js::imports::compile_import_stmt,
        rustdoc::enum_::lower_enum,
        test::{assert_snap, test_db},
    };

    fn compile_test_enum(db: &DbImpl, enum_name: &str) -> String {
        let enum_ = lower_enum(db, db.rustdoc_id(enum_name)).unwrap();
        let mut imports = BTreeSet::new();
        compile_enum_required_imports(db, enum_, &mut imports);

        if imports.is_empty() {
            compile_enum(db, enum_).to_string()
        } else {
            frag!(
                {spaced(imports.into_iter().map(|import| compile_import_stmt(db, import)))}@@
                {compile_enum(db, enum_)}
            )
            .print()
        }
    }

    #[test]
    fn mixed_enum() {
        assert_snap!({
            @lang: "ts";

            let db = test_db! {
                @file: "models.rs";
                pub mod models {
                    #[derive(Clone)]
                    pub struct External;
                }

                pub struct Local;

                /// A shape represented by one of several layouts.
                pub enum Shape {
                    /// A circle with named fields.
                    Circle {
                        /// The circle radius.
                        radius: f64,
                        /// A local Rust-backed value.
                        local: Local,
                        /// An imported Rust-backed value.
                        external: models::External,
                    },
                    /// Coordinates represented by tuple fields.
                    Point(i32, i32),
                    /// A variant without fields.
                    Unit,
                    /// Checks acronym conversion.
                    HTTPRequest,
                }
            };

            compile_test_enum(&db, "Shape")
        });
    }
}
