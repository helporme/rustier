use crate::compiler::{
    codegen::fragment::{FragmentOnce, frag, fragment},
    db::Db,
    ir::rust::{Type, TypeKind},
};

use super::{CastOutput, adapt_to_cast_output, borrowing::render_owned_to_borrowed};

pub fn render_f64_to_type<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    target: Type<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    fragment(move |printer| match target.kind(db) {
        TypeKind::Borrowed(borrowed) => {
            let value = render_f64_to_value_expr(borrowed.inner(db).kind(db), in_expr);
            render_owned_to_borrowed(db, target, value, output).apply(printer);
        }
        target => {
            render_f64_to_value(target, in_expr, output).apply(printer);
        }
    })
}

fn render_f64_to_value<'db, InExpr, OutVar, Continue>(
    target: TypeKind<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let expr = render_f64_to_value_expr(target, in_expr);
    adapt_to_cast_output(expr, output)
}

fn render_f64_to_value_expr<'db, InExpr>(
    target: TypeKind<'db>,
    in_expr: InExpr,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
{
    fragment(move |printer| match target {
        TypeKind::i8
        | TypeKind::i16
        | TypeKind::i32
        | TypeKind::u8
        | TypeKind::u16
        | TypeKind::u32
        | TypeKind::char => {
            let cast = match target {
                TypeKind::i8 => "f64_to_i8",
                TypeKind::i16 => "f64_to_i16",
                TypeKind::i32 => "f64_to_i32",
                TypeKind::u8 => "f64_to_u8",
                TypeKind::u16 => "f64_to_u16",
                TypeKind::u32 => "f64_to_u32",
                TypeKind::char => "f64_to_char",
                _ => unreachable!(),
            };

            frag!(rustier_ext::runtime::casts::{cast}({in_expr})?).apply(printer);
        }
        TypeKind::f32 => frag!(({in_expr})~as f32).apply(printer),
        TypeKind::f64 => in_expr.apply(printer),
        _ => unreachable!("f64 boundary value cannot be cast to `{target:?}`"),
    })
}

pub fn render_type_to_f64<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    source: Type<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let expr = fragment(move |printer| match source.kind(db) {
        TypeKind::char => frag!(({in_expr})~as u32 as f64).apply(printer),
        TypeKind::i8
        | TypeKind::u8
        | TypeKind::i16
        | TypeKind::u16
        | TypeKind::i32
        | TypeKind::u32
        | TypeKind::f32 => frag!(({in_expr})~as f64).apply(printer),
        TypeKind::f64 => in_expr.apply(printer),
        TypeKind::Borrowed(_) => {
            unreachable!("borrowed Rust values are not returned through the interface boundary")
        }
        _ => unreachable!("Rust value `{source:?}` cannot be cast to boundary f64"),
    });

    adapt_to_cast_output(expr, output)
}
