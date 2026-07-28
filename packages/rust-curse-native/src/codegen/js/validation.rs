//! Generates runtime validation for values entering JavaScript facade
//! boundaries.

use std::collections::BTreeSet;

use crate::{
    codegen::{
        db::Db,
        fragment::{Fragment, FragmentOnce, doubles, fragment},
        ir::item::ItemId,
        ir::type_::Type,
        js::{
            enum_::compile_enum_facade_from_binding_expr_required_imports, imports::Import,
            struct_::compile_struct_facade_from_binding_expr_required_imports,
        },
    },
    frag,
};

/// Compiles validation for a facade input value before it crosses into
/// bindings.
pub fn compile_js_facade_value_validation_stmt<'db>(
    db: &'db dyn Db,
    facade_value_type: Type<'db>,
    facade_value_expr: impl Fragment + 'db,
) -> impl FragmentOnce + 'db {
    fragment(move |p| {
        let type_name = match facade_value_type {
            Type::Struct(struct_id) => struct_id.name(db),
            Type::Enum(enum_id) => enum_id.name(db),
            Type::Primitive(_) | Type::Str | Type::String => return,
        };
        let message_start = doubles((
            r#"Expected a value of type \""#,
            type_name,
            r#"\", but value of type \""#,
        ));

        frag!(
            if (!(~{&facade_value_expr}~ instanceof~{type_name}))~{{
                throw new TypeError(
                    {message_start}~ + typeof~{&facade_value_expr}~ + ~{doubles(r#"\" was passed: \""#)}~ + String({&facade_value_expr}) ~+~ {doubles(r#"\""#)}
                );
            }}
        )
        .apply(p);
    })
}

/// Adds imports required by [`compile_js_facade_value_validation_stmt`].
pub fn compile_js_facade_value_validation_stmt_required_imports<'db>(
    db: &'db dyn Db,
    active_mod_id: ItemId<'db>,
    facade_value_type: Type<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    match facade_value_type {
        Type::Struct(struct_id) => {
            compile_struct_facade_from_binding_expr_required_imports(
                db,
                active_mod_id,
                struct_id,
                imports,
            );
        }
        Type::Enum(enum_id) => {
            compile_enum_facade_from_binding_expr_required_imports(
                db,
                active_mod_id,
                enum_id,
                imports,
            );
        }
        Type::Primitive(_) | Type::Str | Type::String => {}
    }
}
