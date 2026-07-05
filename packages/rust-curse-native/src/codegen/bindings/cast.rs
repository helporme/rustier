use std::collections::HashSet;

use crate::codegen::{api::Api, bindings::Printer, db::Db, ir, rustdoc};

/// Compiles shared JavaScript-to-Rust cast helpers used by generated bindings.
#[salsa::tracked(returns(ref))]
pub fn compile_cast_fns(db: &dyn Db) -> String {
    let mut seen = HashSet::new();
    let mut types = Vec::new();

    for mod_id in rustdoc::all_mod_ids(db) {
        for fn_ in rustdoc::lower_mod_fns(db, *mod_id) {
            for param in fn_.params(db) {
                collect_cast_type(*param.type_(db), &mut seen, &mut types);
            }
        }
    }

    let mut out = String::new();
    for type_ in types {
        out.push_str(&compile_cast_fn(db, type_));
    }
    out
}

fn collect_cast_type(type_: ir::Type, seen: &mut HashSet<ir::Type>, types: &mut Vec<ir::Type>) {
    if !seen.insert(type_) {
        return;
    }

    match type_ {
        ir::Type::Primitive(ir::Primitive::Char) => {
            // char is casted to/from u32 when passing a js boundary
            collect_cast_type(ir::Type::Primitive(ir::Primitive::u32), seen, types);
        }
        ir::Type::Primitive(
            ir::Primitive::i8
            | ir::Primitive::i16
            | ir::Primitive::i32
            | ir::Primitive::u8
            | ir::Primitive::u16
            | ir::Primitive::u32
            | ir::Primitive::f32,
        ) => {
            // all "normal" numbers are casted from f64 when passing js border
            collect_cast_type(ir::Type::Primitive(ir::Primitive::f64), seen, types);
        }
        ir::Type::Primitive(ir::Primitive::Boolean | ir::Primitive::f64) | ir::Type::Str => {}
    }

    types.push(type_);
}

fn compile_cast_fn(db: &dyn Db, type_: ir::Type) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    out.write_fmt(format_args!(
        "fn {}(value: {}) -> Result<{}, String> ",
        type_.cast_fn_name(),
        api.js_type(db),
        compile_cast_res(type_)
    ));
    out.curlies(|p| {
        p.lines(&compile_cast_body(db, type_));
    });

    out.blank();
    out.into()
}

fn compile_cast_res(type_: ir::Type) -> &'static str {
    match type_ {
        ir::Type::Primitive(primitive) => primitive.rs_name(),
        ir::Type::Str => "String",
    }
}

fn compile_cast_body(db: &dyn Db, type_: ir::Type) -> String {
    let api = Api::get(db);

    match type_ {
        ir::Type::Primitive(ir::Primitive::Boolean) => api.cast_to_bool(db),
        ir::Type::Primitive(ir::Primitive::Char) => compile_char_cast_body(),
        ir::Type::Str => api.cast_to_string(db),
        ir::Type::Primitive(ir::Primitive::i8) => {
            compile_signed_integer_cast_body(ir::Primitive::i8)
        }
        ir::Type::Primitive(ir::Primitive::u8) => {
            compile_unsigned_integer_cast_body(ir::Primitive::u8)
        }
        ir::Type::Primitive(ir::Primitive::i16) => {
            compile_signed_integer_cast_body(ir::Primitive::i16)
        }
        ir::Type::Primitive(ir::Primitive::u16) => {
            compile_unsigned_integer_cast_body(ir::Primitive::u16)
        }
        ir::Type::Primitive(ir::Primitive::i32) => {
            compile_signed_integer_cast_body(ir::Primitive::i32)
        }
        ir::Type::Primitive(ir::Primitive::u32) => {
            compile_unsigned_integer_cast_body(ir::Primitive::u32)
        }
        ir::Type::Primitive(ir::Primitive::f32) => compile_f32_cast_body(),
        ir::Type::Primitive(ir::Primitive::f64) => api.cast_to_f64(db),
    }
}

fn compile_char_cast_body() -> String {
    let u32_cast_fn = ir::Primitive::u32.cast_fn_name();

    indoc::formatdoc! {r#"
        let value = {u32_cast_fn}(value).map_err(|_| "cannot cast value to char".to_string())?;
        char::from_u32(value).ok_or_else(|| format!("cannot cast {{}} to char", value))
    "#}
}

fn compile_f32_cast_body() -> String {
    let f64_cast_fn = ir::Primitive::f64.cast_fn_name();
    let type_name = ir::Primitive::f32.rs_name();

    indoc::formatdoc! {r#"
        let value = {f64_cast_fn}(value).map_err(|_| "cannot cast value to {type_name}".to_string())?;
        Ok(value as {type_name})
    "#}
}

fn compile_signed_integer_cast_body(primitive: ir::Primitive) -> String {
    let f64_cast_fn = ir::Primitive::f64.cast_fn_name();
    let type_name = primitive.rs_name();

    indoc::formatdoc! {r#"
        let value = {f64_cast_fn}(value).map_err(|_| "cannot cast value to {type_name}".to_string())?;
        if value.trunc() != value || value < {type_name}::MIN as f64 || value > {type_name}::MAX as f64
        {{
        	return Err(format!("cannot cast {{}} to {type_name}", value));
        }}
        Ok(value as {type_name})
    "#}
}

fn compile_unsigned_integer_cast_body(primitive: ir::Primitive) -> String {
    let f64_cast_fn = ir::Primitive::f64.cast_fn_name();
    let type_name = primitive.rs_name();

    indoc::formatdoc! {r#"
        let value = {f64_cast_fn}(value).map_err(|_| "cannot cast value to {type_name}".to_string())?;
        if value.trunc() != value || value < 0 as f64 || value > {type_name}::MAX as f64
        {{
        	return Err(format!("cannot cast {{}} to {type_name}", value));
        }}
        Ok(value as {type_name})
    "#}
}
