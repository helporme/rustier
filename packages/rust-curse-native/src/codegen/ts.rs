//! Compiles rustdoc into TypeScript declarations.

use crate::codegen::{
    db::Db,
    ir::{Fn, Primitive, Type},
    rustdoc,
};

/// Compiles an rustdoc id to ts declaration.
#[salsa::tracked(returns(ref))]
pub fn compile_mod<'db>(db: &'db dyn Db, mod_id: rustdoc::Id<'db>) -> String {
    let mod_ = rustdoc::lower_mod(db, mod_id);
    let fns = rustdoc::lower_mod_fns(db, mod_id);

    let mut out = String::new();

    if let Some(docs) = mod_.docs(db) {
        // todo: move to ambient.d.ts and put a comment on declaration of the module
        // so it's will be shown in IDE when hovering over the module name in import
        out.push_str(&compile_jsdoc(docs));
    }

    for fn_ in fns {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(compile_fn(db, *fn_));
    }

    out
}

/// Compiles an ir function to ts declaration.
#[salsa::tracked(returns(ref))]
fn compile_fn<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> String {
    let mut out = String::new();

    if let Some(docs) = fn_.docs(db) {
        out.push_str(&compile_jsdoc(docs));
    }

    let params = fn_
        .params(db)
        .iter()
        .map(|param| format!("{}: {}", param.name(db), compile_type(param.type_(db))))
        .collect::<Vec<_>>()
        .join(", ");

    let return_type = match fn_.return_type(db) {
        Some(type_) => compile_type(type_),
        None => "void",
    };

    out.push_str(&format!(
        "export function {}({params}): {return_type};\n",
        fn_.name(db)
    ));

    out
}

fn compile_type(type_: Type) -> &'static str {
    match type_ {
        Type::Primitive(primitive) => compile_primitive(primitive),
        Type::Str | Type::String => "string",
    }
}

fn compile_primitive(primitive: Primitive) -> &'static str {
    match primitive {
        Primitive::Boolean => "boolean",
        Primitive::i8
        | Primitive::u8
        | Primitive::i16
        | Primitive::u16
        | Primitive::i32
        | Primitive::u32
        | Primitive::Char
        | Primitive::f32
        | Primitive::f64 => "number",
    }
}

fn compile_jsdoc(docs: &str) -> String {
    let trimmed = docs.trim();
    if trimmed.contains('\n') {
        let lines = trimmed
            .lines()
            .map(|line| format!(" * {}", line.trim()))
            .collect::<Vec<_>>()
            .join("\n");
        format!("/**\n{lines}\n */\n")
    } else {
        format!("/** {trimmed} */\n")
    }
}
