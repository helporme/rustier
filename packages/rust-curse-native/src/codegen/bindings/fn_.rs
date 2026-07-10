//! Module for compiling standalone (not associated) functions into a binding
//! functions that can be exposed to JavaScript.

use crate::codegen::{
    api::Api,
    bindings::{
        boundary::{
            compile_boundary_in_conversion_expr, compile_boundary_in_conversion_stmt,
            compile_boundary_in_type_name, compile_boundary_out_conversion_expr,
            compile_boundary_out_type_name, BoundaryInExpr, BoundaryOutExpr,
        },
        printer::Printer,
    },
    db::Db,
    ir::fn_::Fn,
};

/// Compiles a function into a binding function.
#[salsa::tracked(returns(ref))]
pub fn compile_binding_fn(db: &dyn Db, fn_: Fn<'_>) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    out.lines(&api.fn_header(db, fn_));
    out.write_fmt(format_args!("pub fn {}", fn_.bindings_name(db)));

    out.parens(|out| {
        out.comma_list(fn_.params(db).iter().map(|param| {
            format!(
                "{}: {}",
                param.binding_name(db),
                compile_boundary_in_type_name(db, param.type_(db))
            )
        }));
    });

    let params_as_input_exprs = fn_
        .params(db)
        .iter()
        .map(|p| BoundaryInExpr::from_param(db, *p))
        .collect::<Vec<_>>();

    match fn_.return_type(db) {
        Some(ty) => {
            out.write_fmt(format_args!(
                " -> {} ",
                api.result_type(db, compile_boundary_out_type_name(db, ty))
            ));

            out.curlies(|out| {
                let fn_call_stmt = compile_boundary_in_conversion_expr(
                    db,
                    &params_as_input_exprs,
                    compile_src_fn_call_stmt(db, fn_),
                );

                let conv_expr = compile_boundary_out_conversion_expr(
                    db,
                    &BoundaryOutExpr {
                        in_expr: &fn_call_stmt,
                        type_: ty,
                    },
                );

                out.write("Ok");
                out.parens(|out| out.lines(&conv_expr));
            });
        }
        None => {
            out.write_fmt(format_args!(" -> {} ", api.result_type(db, "()")));

            out.curlies(|out| {
                let fn_call_stmt = compile_boundary_in_conversion_stmt(
                    db,
                    &params_as_input_exprs,
                    compile_src_fn_call_stmt(db, fn_),
                );

                out.lines(&fn_call_stmt);
                out.line("Ok(())");
            });
        }
    }

    out.into()
}

/// Compiles a function into a call statement of the function.
///
/// Assumes the binding parameter variables present in the scope.
fn compile_src_fn_call_stmt<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> String {
    let mut p = Printer::default();

    p.write(fn_.fqn(db));
    p.parens(|p| {
        p.comma_list(fn_.params(db).iter().map(|param| param.binding_name(db)));
    });

    p.into()
}
