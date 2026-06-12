use crate::codegen::{
    bindings::{compile_type, Api, Printer},
    db::Db,
    ir,
};

/// Compiles [`ir::Fn`] to a function bindings using given API.
#[salsa::tracked(returns(ref))]
pub fn compile_fn(db: &dyn Db, api: Api, fn_: ir::Fn<'_>) -> String {
    let mut p = Printer::default();

    p.lines(api.fn_header());
    p.write_fmt(format_args!("pub fn {}", fn_.mangled_name(db)));
    p.parens(|p| {
        p.comma_list(fn_.params(db).iter().map(|param| {
            format!(
                // mark all params as mutable for convenience,
                // so we can always make a mut reference to them
                "mut {}: {}",
                param.mangled_name(db),
                // - numbers that were passed as references are now moved,
                // - &str / &mut str converted to String
                compile_type(param.type_(db))
            )
        }));
    });
    p.write(" -> ");

    let ret_ty = fn_.return_type(db).map(|ty| compile_type(&ty));
    match ret_ty {
        Some(ok_ty) => {
            p.write(&api.result_type(ok_ty));
            p.write(" ");
            p.curlies(|p| {
                p.lines_fmt(format_args!(
                    "let result = {};",
                    compile_fn_call_stmt(db, fn_)
                ));
                p.line("Ok(result)");
            });
        }
        None => {
            p.write(&api.result_type("()"));
            p.write(" ");
            p.curlies(|p| {
                p.lines_fmt(format_args!("{};", compile_fn_call_stmt(db, fn_)));
                p.line("Ok(())");
            });
        }
    }

    p.into()
}

/// Compiles [`ir::Fn`] to a function call statement.
#[salsa::tracked(returns(ref))]
pub fn compile_fn_call_stmt<'db>(db: &'db dyn Db, fn_: ir::Fn<'db>) -> String {
    let mut p = Printer::default();

    p.write(fn_.fqn(db));
    p.parens(|p| {
        p.comma_list(fn_.params(db).iter().map(|param| match param.mode(db) {
            ir::ParamMode::Ref => {
                format!("&{}", param.mangled_name(db))
            }
            ir::ParamMode::Mut => {
                format!("&mut {}", param.mangled_name(db))
            }
            ir::ParamMode::Move => param.mangled_name(db).to_owned(),
        }));
    });

    p.into()
}
