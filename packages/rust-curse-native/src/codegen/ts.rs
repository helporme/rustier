//! Compiles rustdoc into TypeScript declarations.

pub mod sourcemap;

use crate::codegen::{
    bindings::Printer,
    db::Db,
    ir::{Fn, Primitive, Struct, StructField, StructShape, Type},
    rustdoc::{lower_mod, lower_mod_fns, lower_mod_structs, RustdocId},
};

/// Compiles an rustdoc id to ts declaration.
#[salsa::tracked(returns(ref))]
pub fn compile_mod<'db>(db: &'db dyn Db, mod_id: RustdocId<'db>) -> String {
    let mod_ = lower_mod(db, mod_id);

    let mut out = Printer::default();

    if let Some(docs) = mod_.docs(db) {
        // todo: move to ambient.d.ts and put a comment on declaration of the module
        // so it's will be shown in IDE when hovering over the module name in import
        out.lines(&compile_jsdoc(docs));
    }

    for fn_ in lower_mod_fns(db, mod_id) {
        if !out.is_empty() {
            out.blank();
        }
        out.lines(compile_fn(db, *fn_));
    }

    for struct_ in lower_mod_structs(db, mod_id) {
        if !out.is_empty() {
            out.blank();
        }
        out.lines(compile_struct(db, *struct_));
    }

    out.into()
}

/// Compiles an ir function to ts declaration.
#[salsa::tracked(returns(ref))]
fn compile_fn<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> String {
    let mut out = Printer::default();

    if let Some(docs) = fn_.docs(db) {
        out.lines(&compile_jsdoc(docs));
    }

    out.write_fmt(format_args!("export function {}", fn_.name(db)));
    out.parens(|out| {
        out.comma_list(
            fn_.params(db)
                .iter()
                .map(|param| format!("{}: {}", param.name(db), compile_type(param.type_(db)))),
        );
    });

    let return_type = match fn_.return_type(db) {
        Some(type_) => compile_type(type_),
        None => "void",
    };

    out.write_fmt(format_args!(": {};", return_type));

    out.into()
}

/// Compiles an ir struct to ts declaration.
#[salsa::tracked(returns(ref))]
fn compile_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> String {
    let mut out = Printer::default();

    if let Some(docs) = struct_.docs(db) {
        out.lines(&compile_jsdoc(docs));
    }

    out.write_fmt(format_args!("export class {} ", struct_.name(db)));
    out.curlies(|out| {
        match struct_.shape(db) {
            StructShape::Unit => {}
            StructShape::Tuple(fields) => {
                for field in fields.iter().flatten() {
                    out.lines(compile_class_field(db, *field));
                }
            }
            StructShape::Plain { fields, .. } => {
                for field in fields {
                    out.lines(compile_class_field(db, *field));
                }
            }
        }
        out.line("private __rust_curse_brand: never;");
        out.line("private constructor();");
    });

    out.into()
}

#[salsa::tracked(returns(ref))]
fn compile_class_field<'db>(db: &'db dyn Db, field: StructField<'db>) -> String {
    let mut out = Printer::default();

    if let Some(docs) = field.docs(db) {
        out.lines(&compile_jsdoc(docs));
    }
    out.line_fmt(format_args!(
        "public {}: {};",
        field.name(db),
        compile_type(field.type_(db))
    ));

    out.into()
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
