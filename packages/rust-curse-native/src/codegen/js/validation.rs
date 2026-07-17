//! Generates runtime validation for values entering JavaScript facade
//! boundaries.

use std::collections::BTreeSet;

use crate::{
    codegen::{
        db::Db,
        fragment::{Fragment, FragmentOnce, doubles, fragment},
        ir::{IrId, struct_::struct_name_from_rustdoc_id, type_::Type},
        js::{imports::Import, struct_::compile_facade_class_from_binding_expr_required_imports},
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
        let Type::Struct(struct_id) = facade_value_type else {
            return;
        };

        let struct_name = struct_name_from_rustdoc_id(db, struct_id.rustdoc_id(db));
        let message_start = doubles((
            r#"Expected a value of type \""#,
            struct_name,
            r#"\", but value of type \""#,
        ));

        frag!(
            if (!(~{&facade_value_expr}~ instanceof~{struct_name}))~{{
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
    active_mod_id: IrId<'db>,
    facade_value_type: Type<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    if let Type::Struct(struct_id) = facade_value_type {
        compile_facade_class_from_binding_expr_required_imports(
            db,
            active_mod_id,
            struct_id,
            imports,
        );
    }
}
