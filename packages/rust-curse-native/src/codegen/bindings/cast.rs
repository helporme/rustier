//! Module for compiling a cast functions used to convert or reinterpret
//! values passed from the JavaScript to the Rust types expected by the source
//! code.
//!
//! JavaScript value representation is api-specific, see
//! [`crate::codegen::api`].

use std::collections::HashSet;

use crate::codegen::{
    api::Api,
    bindings::Printer,
    db::Db,
    ir::{
        struct_::StructShape,
        type_::{Primitive, Type},
    },
    rustdoc::mod_::{all_mod_ids, lower_mod_fns, lower_mod_structs},
};

/// Compiles all cast helper functions required by public Rust functions and
/// structs.
#[salsa::tracked(returns(ref))]
pub fn compile_cast_fns(db: &dyn Db) -> String {
    let mut seen = HashSet::new();
    let mut types = Vec::new();

    for mod_id in all_mod_ids(db) {
        for fn_ in lower_mod_fns(db, *mod_id) {
            for param in fn_.params(db) {
                collect_cast_type(param.type_(db), &mut seen, &mut types);
            }
        }
        for struct_ in lower_mod_structs(db, *mod_id) {
            match struct_.shape(db) {
                StructShape::Tuple(fields) => {
                    for field in fields.iter().flatten() {
                        collect_cast_type(field.type_(db), &mut seen, &mut types);
                    }
                }
                StructShape::Plain { fields, .. } => {
                    for field in fields {
                        collect_cast_type(field.type_(db), &mut seen, &mut types);
                    }
                }
                StructShape::Unit => {}
            }
        }
    }

    let mut out = String::new();
    for type_ in types {
        out.push_str(&compile_cast_fn(db, type_));
    }
    out
}

/// Collects a cast target type and any wider helper types it depends on.
fn collect_cast_type(type_: Type, seen: &mut HashSet<Type>, types: &mut Vec<Type>) {
    if !seen.insert(type_) {
        return;
    }

    match type_ {
        Type::Str => {
            collect_cast_type(Type::String, seen, types);
            return; // no need for both str and string, we generate one cast fn
                    // for both
        }
        Type::Primitive(Primitive::Char) => {
            // char is casted to/from u32 when passing a js boundary
            collect_cast_type(Type::Primitive(Primitive::u32), seen, types);
        }
        Type::Primitive(
            Primitive::i8
            | Primitive::i16
            | Primitive::i32
            | Primitive::u8
            | Primitive::u16
            | Primitive::u32
            | Primitive::f32,
        ) => {
            // all "normal" numbers are casted from f64 when passing js border
            collect_cast_type(Type::Primitive(Primitive::f64), seen, types);
        }
        Type::Primitive(Primitive::Boolean | Primitive::f64) | Type::String => {}
    }

    types.push(type_);
}

/// Compiles a cast helper function for the given type.
fn compile_cast_fn(db: &dyn Db, type_: Type) -> String {
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

/// Compiles the successful `Result` type of a cast function.
///
/// ```ignore (incomplete expression)
/// fn cast(value: JsValue) -> Result<String, String>
///                                // ~~~~~~
///                                // this part
/// ```
fn compile_cast_res(type_: Type) -> &'static str {
    match type_ {
        Type::Primitive(primitive) => primitive.rs_name(),
        Type::Str | Type::String => "String",
    }
}

/// Compiles the body of a cast function.
fn compile_cast_body(db: &dyn Db, type_: Type) -> String {
    let api = Api::get(db);

    match type_ {
        Type::Primitive(Primitive::Boolean) => api.cast_to_bool(db),
        Type::Primitive(Primitive::Char) => compile_char_cast_body(),
        Type::Str | Type::String => api.cast_to_string(db),
        Type::Primitive(Primitive::i8) => compile_signed_integer_cast_body(Primitive::i8),
        Type::Primitive(Primitive::u8) => compile_unsigned_integer_cast_body(Primitive::u8),
        Type::Primitive(Primitive::i16) => compile_signed_integer_cast_body(Primitive::i16),
        Type::Primitive(Primitive::u16) => compile_unsigned_integer_cast_body(Primitive::u16),
        Type::Primitive(Primitive::i32) => compile_signed_integer_cast_body(Primitive::i32),
        Type::Primitive(Primitive::u32) => compile_unsigned_integer_cast_body(Primitive::u32),
        Type::Primitive(Primitive::f32) => compile_f32_cast_body(),
        Type::Primitive(Primitive::f64) => api.cast_to_f64(db),
    }
}

/// Compiles a cast body that converts a JavaScript number into a Rust `char`.
fn compile_char_cast_body() -> String {
    let u32_cast_fn = Primitive::u32.cast_fn_name();

    indoc::formatdoc! {r#"
        let value = {u32_cast_fn}(value).map_err(|_| "cannot cast value to char".to_string())?;
        char::from_u32(value).ok_or_else(|| format!("cannot cast {{}} to char", value))
    "#}
}

/// Compiles a cast body that converts a JavaScript number into a Rust `f32`.
fn compile_f32_cast_body() -> String {
    let f64_cast_fn = Primitive::f64.cast_fn_name();
    let type_name = Primitive::f32.rs_name();

    indoc::formatdoc! {r#"
        let value = {f64_cast_fn}(value).map_err(|_| "cannot cast value to {type_name}".to_string())?;
        Ok(value as {type_name})
    "#}
}

/// Compiles a cast body that converts a JavaScript number into a signed Rust
/// integer.
fn compile_signed_integer_cast_body(primitive: Primitive) -> String {
    let f64_cast_fn = Primitive::f64.cast_fn_name();
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

/// Compiles a cast body that converts a JavaScript number into an unsigned Rust
/// integer.
fn compile_unsigned_integer_cast_body(primitive: Primitive) -> String {
    let f64_cast_fn = Primitive::f64.cast_fn_name();
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
