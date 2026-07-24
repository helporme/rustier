use std::collections::BTreeSet;

use crate::{
    codegen::{
        db::Db,
        fragment::{FragmentOnce, FragmentPrint, fragment},
        ir::{
            shape::ShapeField,
            struct_::{Struct, struct_name_from_ir_id},
            type_::Type,
        },
        js::imports::{Import, ImportFrom},
        ts::jsdoc::compile_jsdoc,
    },
    frag,
};

/// Compiles an IR struct into a TypeScript class declaration.
///
/// Requires imports added by [`compile_struct_required_imports`] to be in
/// scope.
#[salsa::tracked(returns(ref))]
pub fn compile_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> String {
    let fields = fragment(move |p| {
        let fields = struct_.fields(db);

        for (index, field) in fields.iter().enumerate() {
            if index > 0 {
                "\n".apply(p);
            }
            compile_class_field(db, *field).apply(p);
        }

        if !fields.is_empty() {
            "\n".apply(p);
        }
        frag!(
            private __rust_curse_brand: never;@
            private constructor();
        )
        .apply(p);
    });

    frag!(
        {compile_jsdoc(struct_.docs(db).as_deref())}
        export class ~{struct_.name(db)}~{{ {fields} }}
    )
    .print()
}

/// Adds imports required by [`compile_struct`].
pub fn compile_struct_required_imports<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    let active_mod_id = struct_.id(db).parent_mod_id(db).unwrap();

    for field in struct_.fields(db).iter() {
        let (type_id, type_name) = match field.type_(db) {
            Type::Struct(struct_id) => (struct_id, struct_name_from_ir_id(db, struct_id)),
            Type::Enum(enum_id) => (enum_id.inner(db), enum_id.name(db)),
            Type::Primitive(_) | Type::Str | Type::String => continue,
        };
        let type_mod_id = type_id.parent_mod_id(db).unwrap();

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

fn compile_class_field<'db>(db: &'db dyn Db, field: ShapeField<'db>) -> impl FragmentOnce + 'db {
    let readonly = if field.is_readonly(db) {
        "readonly "
    } else {
        ""
    };

    frag!(
        {compile_jsdoc(field.docs(db).as_deref())}
        public ~{readonly}{field.name(db)}:~{field.type_(db).ts_name(db)};
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        db::DbImpl,
        fragment::{FragmentPrint, spaced},
        js::imports::compile_import_stmt,
        rustdoc::struct_::lower_struct,
        test::{assert_snap, test_db},
    };

    fn compile_test_struct(db: &DbImpl, struct_name: &str) -> String {
        let struct_ = lower_struct(db, db.rustdoc_id(struct_name)).unwrap();
        let mut imports = BTreeSet::new();
        compile_struct_required_imports(db, struct_, &mut imports);

        if imports.is_empty() {
            compile_struct(db, struct_).to_string()
        } else {
            frag!(
                {spaced(imports.into_iter().map(|import| compile_import_stmt(db, import)))}@@
                {compile_struct(db, struct_)}
            )
            .print()
        }
    }

    #[test]
    fn class_from_plain_struct() {
        assert_snap!({
            @lang: "ts";

            let db = test_db! {
                @file: "models.rs";
                pub mod models {
                    pub struct External;
                }

                pub struct Local;

                /// Stores primitive, local, and external values.
                pub struct Foo {
                    /// A numeric value.
                    pub number: i32,
                    /// A text value.
                    pub text: String,
                    /// A value declared in the same module.
                    pub local: Local,
                    /// A value imported from another module.
                    pub external: models::External,
                }
            };

            compile_test_struct(&db, "Foo")
        });
    }

    #[test]
    fn class_from_tuple_struct() {
        assert_snap!({
            @lang: "ts";

            let db = test_db! {
                pub struct Foo(pub i32, pub String);
            };

            compile_test_struct(&db, "Foo")
        });
    }

    #[test]
    fn class_from_unit_struct() {
        assert_snap!({
            @lang: "ts";

            let db = test_db! {
                pub struct Foo;
                pub struct Bar();
                pub struct Baz {}
            };

            frag!(
                {compile_test_struct(&db, "Foo")}@@
                {compile_test_struct(&db, "Bar")}@@
                {compile_test_struct(&db, "Baz")}
            )
            .print()
        });
    }
}
