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
    let mut out = Printer::default();

    out.lines(&api.fn_header(db, fn_));
    out.write_fmt(format_args!("pub fn {}", fn_.mangled_name(db)));
    out.parens(|p| {
        p.comma_list(
            fn_.params(db)
                .iter()
                .map(|param| compile_param_decl(db, *param)),
        );
    });
    out.write(" -> ");

    let ret_ty = fn_.return_type(db).map(|ty| compile_ret_decl(ty));
    match ret_ty {
        Some(ok_ty) => {
            out.write(&api.result_type(db, ok_ty));
            out.write(" ");
            out.curlies(|p| {
                for param in fn_.params(db) {
                    p.lines(compile_param_conv(db, *param));
                }
                p.lines_fmt(format_args!(
                    "let result = {};",
                    compile_fn_call_stmt(db, fn_)
                ));
                if let Some(return_type) = fn_.return_type(db) {
                    p.lines(&compile_ret_conv(db, return_type));
                }
                p.line("Ok(result)");
            });
        }
        None => {
            out.write(&api.result_type(db, "()"));
            out.write(" ");
            out.curlies(|p| {
                for param in fn_.params(db) {
                    p.lines(compile_param_conv(db, *param));
                }
                p.lines_fmt(format_args!("{};", compile_fn_call_stmt(db, fn_)));
                p.line("Ok(())");
            });
        }
    }

    out.into()
}

/// Compiles a function parameter into a param declaration.
///
/// ```ignore (incomplete expression)
/// pub fn foo(value: String)
///         // ~~~~~~~~~~~~~
///         // this part
/// ```
#[salsa::tracked(returns(ref))]
pub fn compile_param_decl<'db>(db: &'db dyn Db, param: ir::FnParam<'db>) -> String {
    let api = Api::get(db);
    format!("{}: {}", param.mangled_name(db), api.js_type(db))
}

/// Compiles Rust boundary conversion statements for a generated binding
/// parameter.
///
/// ```ignore (incomplete expression)
/// pub fn foo(param_name: String) {
///    let mut param_name = /* conversion */;
///  // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
///  // this part
/// }
/// ```
#[salsa::tracked(returns(ref))]
pub fn compile_param_conv<'db>(db: &'db dyn Db, param: ir::FnParam<'db>) -> String {
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
            "format!(\"Invalid argument `{}`: expected Rust `{}` from a JavaScript {}: {{}}\", err)",
            param.name(db),
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

/// Compiles the return type of a function into a return type declaration.
///
/// ```ignore (incomplete expression)
/// pub fn foo() -> Result<String>
///                     // ~~~~~~
///                     // this part
/// ```
pub fn compile_ret_decl(ty: ir::Type) -> &'static str {
    compile_type(&ty)
}

/// Compiles Rust boundary conversion statements for a generated binding return
/// value.
///
/// ```ignore (incomplete expression)
/// pub fn foo(param_name: String) {
///    let result = get_my_result();
///
///    let result = result as ...;
/// // ~~~~~~~~~~~~~~~~~~~~~~~~~~~
/// // this part
/// }
/// ```
pub fn compile_ret_conv(db: &dyn Db, ty: ir::Type) -> String {
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
