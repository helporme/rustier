//! Module for compiling standalone (not associated) functions into a binding
//! functions that can be exposed to JavaScript.

use crate::codegen::{
    api::Api,
    bindings::printer::Printer,
    db::Db,
    ir::{
        fn_::{Fn, FnParam, ParamMode},
        type_::{Primitive, Type},
    },
};

/// Compiles a function into a binding function.
#[salsa::tracked(returns(ref))]
pub fn compile_binding_fn(db: &dyn Db, fn_: Fn<'_>) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    out.lines(&api.fn_header(db, fn_));
    out.write_fmt(format_args!("pub fn {}", fn_.bindings_name(db)));
    out.parens(|p| {
        p.comma_list(
            fn_.params(db)
                .iter()
                .map(|param| compile_binding_fn_param_decl(db, *param)),
        );
    });
    out.write(" -> ");

    let ret_ty = fn_
        .return_type(db)
        .map(|ty| compile_binding_fn_ret_decl(ty));
    match ret_ty {
        Some(ok_ty) => {
            out.write(&api.result_type(db, ok_ty));
            out.write(" ");
            out.curlies(|p| {
                for param in fn_.params(db) {
                    p.lines(compile_binding_fn_param_conv(db, *param));
                }
                p.lines_fmt(format_args!(
                    "let result = {};",
                    compile_src_fn_call_stmt(db, fn_)
                ));
                if let Some(return_type) = fn_.return_type(db) {
                    p.lines(&compile_binding_fn_ret_conv(db, return_type));
                }
                p.line("Ok(result)");
            });
        }
        None => {
            out.write(&api.result_type(db, "()"));
            out.write(" ");
            out.curlies(|p| {
                for param in fn_.params(db) {
                    p.lines(compile_binding_fn_param_conv(db, *param));
                }
                p.lines_fmt(format_args!("{};", compile_src_fn_call_stmt(db, fn_)));
                p.line("Ok(())");
            });
        }
    }

    out.into()
}

/// Compiles a function parameter into a param declaration of a binding
/// function.
///
/// ```ignore (incomplete expression)
/// pub fn foo(value: String)
///         // ~~~~~~~~~~~~~
///         // this part
/// ```
#[salsa::tracked(returns(ref))]
pub fn compile_binding_fn_param_decl<'db>(db: &'db dyn Db, param: FnParam<'db>) -> String {
    let api = Api::get(db);
    format!("{}: {}", param.binding_name(db), api.js_type(db))
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
pub fn compile_binding_fn_param_conv<'db>(db: &'db dyn Db, param: FnParam<'db>) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    let name = param.binding_name(db);
    let mutable = matches!(param.mode(db), ParamMode::Mut)
        .then_some("mut ")
        .unwrap_or("");

    let type_ = param.type_(db);
    let error = api.error(
        db,
        &format!(
            "format!(\"Invalid argument `{}`: expected Rust `{}` from a JavaScript {}: {{}}\", err)",
            param.name(db),
            type_.rs_name(),
            type_.ts_name()
        ),
    );

    out.line_fmt(format_args!(
        "let {mutable}{name} = {}({name}).map_err(|err| {error})?;",
        type_.cast_fn_name()
    ));

    out.into()
}

/// Compiles the return type of a function into a return type declaration for a
/// binding function.
///
/// ```ignore (incomplete expression)
/// pub fn foo() -> Result<String>
///                     // ~~~~~~
///                     // this part
/// ```
pub fn compile_binding_fn_ret_decl(ty: Type) -> &'static str {
    match ty {
        Type::Primitive(Primitive::Boolean) => "bool",
        Type::Primitive(Primitive::i8) => "i8",
        Type::Primitive(Primitive::u8) => "u8",
        Type::Primitive(Primitive::i16) => "i16",
        Type::Primitive(Primitive::u16) => "u16",
        Type::Primitive(Primitive::i32) => "i32",
        Type::Primitive(Primitive::u32) | Type::Primitive(Primitive::Char) => "u32",
        Type::Primitive(Primitive::f64) | Type::Primitive(Primitive::f32) => "f64",
        Type::Str | Type::String => "String",
    }
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
pub fn compile_binding_fn_ret_conv(db: &dyn Db, ty: Type) -> String {
    let _ = db;
    let mut p = Printer::default();

    match ty {
        Type::Primitive(Primitive::Char) => p.line("let result = result as u32;"),
        Type::Primitive(Primitive::f32) => p.line("let result = result as f64;"),
        _ => {}
    }

    p.into()
}

/// Compiles a function into a call statement of the function.
///
/// Assumes the binding parameter variables present in the scope.
#[salsa::tracked(returns(ref))]
fn compile_src_fn_call_stmt<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> String {
    let mut p = Printer::default();

    p.write(fn_.fqn(db));
    p.parens(|p| {
        p.comma_list(fn_.params(db).iter().map(|param| match param.mode(db) {
            ParamMode::Ref => {
                format!("&{}", param.binding_name(db))
            }
            ParamMode::Mut => {
                format!("&mut {}", param.binding_name(db))
            }
            ParamMode::Move => param.binding_name(db).to_owned(),
        }));
    });

    p.into()
}
