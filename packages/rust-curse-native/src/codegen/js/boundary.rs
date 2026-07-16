use std::collections::BTreeSet;

use crate::codegen::{
    db::Db,
    fragment::{FragmentOnce, fragment},
    ir::{IrId, type_::Type},
    js::{
        imports::Import,
        struct_::{
            compile_binding_class_from_facade_class_expr,
            compile_binding_class_to_facade_class_expr,
            compile_binding_class_to_facade_class_required_imports,
        },
    },
};

/// Compiles a boundary value expression to a facade value expression.
///
/// Requires to [`compile_js_boundary_in_required_imports`] imports to be in
/// scope.
pub fn compile_js_boundary_in_to_facade_type_expr<'db>(
    db: &'db dyn Db,
    binding_value_type: Type<'db>,
    binding_value_expr: impl FragmentOnce + 'db,
) -> impl FragmentOnce + 'db {
    fragment(move |p| match binding_value_type {
        Type::Struct(struct_id) => {
            compile_binding_class_to_facade_class_expr(db, struct_id, binding_value_expr).apply(p);
        }
        Type::Primitive(_) | Type::Str | Type::String => binding_value_expr.apply(p),
    })
}

/// Returns the required imports for
/// [`compile_js_boundary_out_from_facade_type_expr`].
pub fn compile_js_boundary_out_from_facade_type_expr_required_imports<'db>(
    db: &'db dyn Db,
    active_mod_id: IrId<'db>,
    binding_value_type: Type<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    match binding_value_type {
        Type::Struct(struct_id) => {
            compile_binding_class_to_facade_class_required_imports(
                db,
                active_mod_id,
                struct_id,
                imports,
            );
        }
        Type::Primitive(_) | Type::Str | Type::String => {}
    }
}

/// Compiles a facade value expression to a boundary value expression.
pub fn compile_js_boundary_out_from_facade_type_expr<'a>(
    facade_value_type: Type<'a>,
    facade_value_expr: impl FragmentOnce + 'a,
) -> impl FragmentOnce + 'a {
    fragment(move |p| match facade_value_type {
        Type::Struct(_) => {
            compile_binding_class_from_facade_class_expr(facade_value_expr).apply(p);
        }
        Type::Primitive(_) | Type::Str | Type::String => facade_value_expr.apply(p),
    })
}
