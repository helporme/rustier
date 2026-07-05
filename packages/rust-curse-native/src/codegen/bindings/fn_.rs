use crate::codegen::{
    api::Api,
    bindings::{compile_type, Printer},
    db::Db,
    ir,
};

/// Compiles [`ir::Fn`] to function bindings using the active API.
#[salsa::tracked(returns(ref))]
pub fn compile_fn(db: &dyn Db, fn_: ir::Fn<'_>) -> String {
    let api = Api::get(db);
    let mut p = Printer::default();

    p.lines(&api.fn_header(db, fn_));
    p.write_fmt(format_args!("pub fn {}", fn_.mangled_name(db)));
    p.parens(|p| {
        p.comma_list(
            fn_.params(db)
                .iter()
                .map(|param| format!("{}: {}", param.mangled_name(db), api.js_type(db))),
        );
    });
    p.write(" -> ");

    let ret_ty = fn_.return_type(db).map(|ty| compile_type(&ty));
    match ret_ty {
        Some(ok_ty) => {
            p.write(&api.result_type(db, ok_ty));
            p.write(" ");
            p.curlies(|p| {
                for param in fn_.params(db) {
                    p.lines(compile_param_conv(db, fn_, *param));
                }
                p.lines_fmt(format_args!(
                    "let result = {};",
                    compile_fn_call_stmt(db, fn_)
                ));
                if let Some(return_type) = fn_.return_type(db) {
                    p.lines(&compile_ret_value_conv(db, return_type));
                }
                p.line("Ok(result)");
            });
        }
        None => {
            p.write(&api.result_type(db, "()"));
            p.write(" ");
            p.curlies(|p| {
                for param in fn_.params(db) {
                    p.lines(compile_param_conv(db, fn_, *param));
                }
                p.lines_fmt(format_args!("{};", compile_fn_call_stmt(db, fn_)));
                p.line("Ok(())");
            });
        }
    }

    p.into()
}

/// Compiles Rust boundary conversion statements for a generated binding
/// parameter.
#[salsa::tracked(returns(ref))]
pub fn compile_param_conv<'db>(
    db: &'db dyn Db,
    fn_: ir::Fn<'db>,
    param: ir::FnParam<'db>,
) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    let name = param.mangled_name(db);
    let mutable = matches!(param.mode(db), ir::ParamMode::Mut)
        .then_some("mut ")
        .unwrap_or("");

    let type_ = param.type_(db);
    let error = api.error(
        db,
        &format!(
            "format!(\"Invalid argument `{}` passed to `{}`: expected Rust `{}` from a JavaScript {}: {{}}\", err)",
            param.name(db),
            fn_.name(db),
            type_.rs_name(),
            type_.js_name()
        ),
    );

    out.line_fmt(format_args!(
        "let {mutable}{name} = {}({name}).map_err(|err| {error})?;",
        type_.cast_fn_name()
    ));

    out.into()
}

/// Compiles Rust boundary conversion statements for a generated binding return
/// value.
pub fn compile_ret_value_conv(db: &dyn Db, ty: ir::Type) -> String {
    let _ = db;
    let mut p = Printer::default();

    match ty {
        ir::Type::Primitive(ir::Primitive::Char) => p.line("let result = result as u32;"),
        ir::Type::Primitive(ir::Primitive::f32) => p.line("let result = result as f64;"),
        _ => {}
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
