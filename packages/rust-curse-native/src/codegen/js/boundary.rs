use std::collections::BTreeSet;

use crate::codegen::{
    db::Db,
    fragment::{FragmentOnce, fragment},
    ir::{IrId, type_::Type},
    js::{
        enum_::{
            compile_binding_class_from_facade_expr as compile_binding_enum_from_facade_expr,
            compile_facade_class_from_binding_expr as compile_enum_facade_from_binding_expr,
            compile_facade_class_from_binding_expr_required_imports as compile_enum_facade_from_binding_expr_required_imports,
        },
        imports::Import,
        struct_::{
            compile_binding_class_from_facade_expr as compile_binding_struct_from_facade_expr,
            compile_facade_class_from_binding_expr as compile_struct_facade_from_binding_expr,
            compile_facade_class_from_binding_expr_required_imports as compile_struct_facade_from_binding_expr_required_imports,
        },
    },
};

/// Compiles a binding value expression into a facade value expression.
///
/// Requires imports added by
/// [`compile_js_binding_value_to_facade_expr_required_imports`] to be in scope.
pub fn compile_js_binding_value_to_facade_expr<'db>(
    db: &'db dyn Db,
    binding_value_type: Type<'db>,
    binding_value_expr: impl FragmentOnce + 'db,
) -> impl FragmentOnce + 'db {
    fragment(move |p| match binding_value_type {
        Type::Struct(struct_id) => {
            compile_struct_facade_from_binding_expr(db, struct_id, binding_value_expr).apply(p);
        }
        Type::Enum(enum_id) => {
            compile_enum_facade_from_binding_expr(db, enum_id, binding_value_expr).apply(p);
        }
        Type::Primitive(_) | Type::Str | Type::String => binding_value_expr.apply(p),
    })
}

/// Adds the imports required by [`compile_js_binding_value_to_facade_expr`].
pub fn compile_js_binding_value_to_facade_expr_required_imports<'db>(
    db: &'db dyn Db,
    active_mod_id: IrId<'db>,
    binding_value_type: Type<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    match binding_value_type {
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

/// Compiles a facade value expression into a binding value expression.
pub fn compile_js_facade_value_to_binding_expr<'a>(
    facade_value_type: Type<'a>,
    facade_value_expr: impl FragmentOnce + 'a,
) -> impl FragmentOnce + 'a {
    fragment(move |p| match facade_value_type {
        Type::Struct(_) => compile_binding_struct_from_facade_expr(facade_value_expr).apply(p),
        Type::Enum(_) => compile_binding_enum_from_facade_expr(facade_value_expr).apply(p),
        Type::Primitive(_) | Type::Str | Type::String => facade_value_expr.apply(p),
    })
}
