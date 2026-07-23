use std::collections::BTreeSet;

use crate::{
    codegen::{
        db::Db,
        fragment::{FragmentOnce, fragment},
        ir::{IrId, type_::Type},
        js::{
            enum_::{
                compile_binding_enum_from_facade_expr, compile_enum_facade_from_binding_expr,
                compile_enum_facade_from_binding_expr_required_imports,
            },
            imports::Import,
            struct_::{
                compile_binding_struct_from_facade_expr, compile_struct_facade_from_binding_expr,
                compile_struct_facade_from_binding_expr_required_imports,
            },
            validation::{
                compile_js_facade_value_validation_stmt,
                compile_js_facade_value_validation_stmt_required_imports,
            },
        },
    },
    frag,
};

/// Compiles a statement that assigns a binding value converted to its facade
/// representation to an output variable.
///
/// `out_var_name` must refer to an existing assignment target.
///
/// Requires imports added by
/// [`compile_js_binding_value_to_facade_stmt_required_imports`] to be in scope.
pub fn compile_js_binding_value_to_facade_stmt<'db>(
    db: &'db dyn Db,
    binding_value_type: Type<'db>,
    binding_value_expr: impl FragmentOnce + 'db,
    out_var_name: impl FragmentOnce + 'db,
) -> impl FragmentOnce + 'db {
    let facade_value_expr = fragment(move |p| match binding_value_type {
        Type::Struct(struct_id) => {
            compile_struct_facade_from_binding_expr(db, struct_id, binding_value_expr).apply(p);
        }
        Type::Enum(enum_id) => {
            compile_enum_facade_from_binding_expr(db, enum_id, binding_value_expr).apply(p);
        }
        Type::Primitive(_) | Type::Str | Type::String => binding_value_expr.apply(p),
    });

    frag!({out_var_name}~ = ~{facade_value_expr};)
}

/// Adds the imports required by [`compile_js_binding_value_to_facade_stmt`].
pub fn compile_js_binding_value_to_facade_stmt_required_imports<'db>(
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

/// Compiles a statement that validates and assigns a facade value converted to
/// its binding representation to an output variable.
///
/// `out_var_name` must refer to an existing assignment target.
///
/// Requires imports added by
/// [`compile_js_facade_value_to_binding_stmt_required_imports`] to be in scope.
pub fn compile_js_facade_value_to_binding_stmt<'db>(
    db: &'db dyn Db,
    facade_value_type: Type<'db>,
    facade_value_expr: impl FragmentOnce + 'db,
    out_var_name: impl FragmentOnce + 'db,
) -> impl FragmentOnce + 'db {
    const FACADE_VALUE: &str = "__facade_value";

    fragment(move |p| match facade_value_type {
        Type::Struct(_) | Type::Enum(_) => {
            let binding_value_expr = fragment(move |p| match facade_value_type {
                Type::Struct(_) => {
                    compile_binding_struct_from_facade_expr(FACADE_VALUE).apply(p);
                }
                Type::Enum(_) => {
                    compile_binding_enum_from_facade_expr(FACADE_VALUE).apply(p);
                }
                Type::Primitive(_) | Type::Str | Type::String => unreachable!(),
            });

            frag!(
                {{
                    const ~{FACADE_VALUE}~ = ~{facade_value_expr};@
                    {compile_js_facade_value_validation_stmt(db, facade_value_type, FACADE_VALUE)}@
                    {out_var_name}~ = ~{binding_value_expr};
                }}
            )
            .apply(p);
        }
        Type::Primitive(_) | Type::Str | Type::String => {
            frag!({out_var_name}~ = ~{facade_value_expr};).apply(p);
        }
    })
}

/// Adds the imports required by
/// [`compile_js_facade_value_to_binding_stmt`].
pub fn compile_js_facade_value_to_binding_stmt_required_imports<'db>(
    db: &'db dyn Db,
    active_mod_id: IrId<'db>,
    facade_value_type: Type<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    compile_js_facade_value_validation_stmt_required_imports(
        db,
        active_mod_id,
        facade_value_type,
        imports,
    );
}
