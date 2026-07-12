//! Module for compiling standalone (not associated) functions into a binding
//! functions that can be exposed to JavaScript.

use crate::codegen::{
    api::Api,
    bindings::boundary::{
        BoundaryInExpr, compile_boundary_in_conversion_expr, compile_boundary_in_conversion_stmt,
        compile_boundary_in_type_name, compile_boundary_out_conversion_expr,
        compile_boundary_out_type_name,
    },
    db::Db,
    fragment::{FragmentOnce, FragmentPrint, comma_list, frag, fragment},
    ir::fn_::Fn,
};

/// Compiles a function into a binding function.
#[salsa::tracked(returns(ref))]
pub fn compile_binding_fn<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> String {
    let api = Api::get(db);
    let result_type = match fn_.return_type(db) {
        Some(type_) => api.result_type(db, compile_boundary_out_type_name(db, type_)),
        None => api.result_type(db, "()"),
    };

    frag!(
        {api.fn_header(db, fn_)}@
        pub fn ~{fn_.bindings_name(db)}({compile_binding_fn_params(db, fn_)})~ -> ~{result_type}~ {{
            {compile_binding_fn_body(db, fn_)}
        }}
    )
    .print()
}

fn compile_binding_fn_params<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> impl FragmentOnce + 'db {
    frag!({
        comma_list(fn_.params(db).iter().map(|param| {
            frag!(
                {param.binding_name(db)}:~
                {compile_boundary_in_type_name(db, param.type_(db))}
            )
        }))
    })
}

fn compile_binding_fn_body<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> impl FragmentOnce + 'db {
    fragment(move |p| {
        let params_as_input_exprs = fn_
            .params(db)
            .iter()
            .map(|param| BoundaryInExpr::from_param(db, *param));

        match fn_.return_type(db) {
            Some(type_) => {
                let fn_call_expr = compile_boundary_in_conversion_expr(
                    db,
                    params_as_input_exprs,
                    compile_src_fn_call_stmt(db, fn_),
                );
                let conversion_expr = compile_boundary_out_conversion_expr(db, fn_call_expr, type_);

                frag!(Ok({ conversion_expr })).apply(p);
            }
            None => {
                let fn_call_stmt = compile_boundary_in_conversion_stmt(
                    db,
                    params_as_input_exprs,
                    frag!({compile_src_fn_call_stmt(db, fn_)};),
                );

                frag!(
                    {fn_call_stmt}@
                    Ok(())
                )
                .apply(p);
            }
        }
    })
}

/// Compiles a function into a call statement of the function.
///
/// Assumes the binding parameter variables present in the scope.
fn compile_src_fn_call_stmt<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> impl FragmentOnce + 'db {
    let params = fragment(move |printer| {
        if fn_.params(db).is_empty() {
            "()".apply(printer);
        } else {
            frag!(({ comma_list(fn_.params(db).iter().map(|param| param.binding_name(db))) }))
                .apply(printer);
        }
    });

    frag!({fn_.fqn(db)}{params})
}
