use crate::codegen::{
    bindings::{compile_type, Api, Printer},
    db::Db,
    ir,
};

/// Compiles [`ir::Fn`] to a function bindings using given API.
#[salsa::tracked(returns(ref))]
pub fn compile_fn(db: &dyn Db, api: Api, fn_: ir::Fn<'_>) -> String {
    let mut p = Printer::default();

    p.lines(&api.fn_header(db, fn_));
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
                for param in fn_.params(db) {
                    p.lines(compile_param_conv(db, api, *param));
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
            p.write(&api.result_type("()"));
            p.write(" ");
            p.curlies(|p| {
                for param in fn_.params(db) {
                    p.lines(compile_param_conv(db, api, *param));
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
pub fn compile_param_conv<'db>(db: &'db dyn Db, api: Api, param: ir::FnParam<'db>) -> String {
    let mut p = Printer::default();
    let mutable = matches!(param.mode(db), ir::ParamMode::Mut)
        .then_some("mut ")
        .unwrap_or("");

    match param.type_(db) {
        // char is passed as a number from the js side and converted to u32 by api.
        // Not all u32 are valid unicode code points however.
        ir::Type::Primitive(ir::Primitive::Char) => {
            let name = param.mangled_name(db);
            p.line_fmt(format_args!(
                "let {mutable}{name} = char::from_u32({name}).ok_or_else(|| {{"
            ));
            p.line_fmt(format_args!(
                "\t{}",
                api.error(&format!(
                    "format!(\"invalid char code point: {{}}\", {name})"
                ))
            ));
            p.line("})?;");
        }
        // we coerce f64 to f32 on the js side with some loss of precision
        ir::Type::Primitive(ir::Primitive::f32) => {
            let name = param.mangled_name(db);
            p.line_fmt(format_args!("let {mutable}{name} = {name} as f32;"));
        }
        _ => {}
    }

    p.into()
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
