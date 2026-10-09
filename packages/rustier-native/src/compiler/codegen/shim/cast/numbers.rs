use crate::compiler::{
    codegen::{
        fragment::{FragmentOnce, doubles, frag, fragment},
        shim::Platform,
    },
    db::Db,
    ir::rust::{Type, TypeKind},
};

use super::{CastOutput, adapt_to_cast_output, borrowing::render_owned_to_borrowed};

pub fn render_f64_to_type<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    platform: Platform,
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
            let value = render_f64_to_value_expr(platform, borrowed.inner(db).kind(db), in_expr);
            render_owned_to_borrowed(db, target, value, output).apply(printer);
        }
        target => {
            render_f64_to_value(platform, target, in_expr, output).apply(printer);
        }
    })
}

fn render_f64_to_value<'db, InExpr, OutVar, Continue>(
    platform: Platform,
    target: TypeKind<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let expr = render_f64_to_value_expr(platform, target, in_expr);
    adapt_to_cast_output(expr, output)
}

fn render_f64_to_value_expr<'db, InExpr>(
    platform: Platform,
    target: TypeKind<'db>,
    in_expr: InExpr,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
{
    fragment(move |printer| match target {
        TypeKind::i8 | TypeKind::i16 | TypeKind::i32 => {
            render_f64_to_signed_expr(platform, target, in_expr).apply(printer);
        }
        TypeKind::u8 | TypeKind::u16 | TypeKind::u32 => {
            render_f64_to_unsigned_expr(platform, target, in_expr).apply(printer);
        }
        TypeKind::char => {
            render_f64_to_char_expr(platform, in_expr).apply(printer);
        }
        TypeKind::f32 => frag!(({in_expr})~as f32).apply(printer),
        TypeKind::f64 => in_expr.apply(printer),
        _ => unreachable!("f64 boundary value cannot be cast to `{target:?}`"),
    })
}

fn render_f64_to_signed_expr<'db, InExpr>(
    platform: Platform,
    target: TypeKind<'db>,
    in_expr: InExpr,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
{
    let type_name = match target {
        TypeKind::i8 => "i8",
        TypeKind::i16 => "i16",
        TypeKind::i32 => "i32",
        _ => unreachable!("signed integer cast requires a signed integer target"),
    };
    frag!({{
        let type_name = ~{doubles(type_name)};@
        let value = ~{in_expr};@

        if value.trunc()~ != value || value < ~{type_name}::MIN as f64 || value > ~{type_name}::MAX as f64 ~{{
            return Err({
                platform.error(frag!(
                    format!("cannot cast {value} to {type_name}")
                ))
            });
        }}@

        value as ~{type_name}
    }})
}

fn render_f64_to_unsigned_expr<'db, InExpr>(
    platform: Platform,
    target: TypeKind<'db>,
    in_expr: InExpr,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
{
    let type_name = match target {
        TypeKind::u8 => "u8",
        TypeKind::u16 => "u16",
        TypeKind::u32 => "u32",
        _ => unreachable!("unsigned integer cast requires an unsigned integer target"),
    };
    frag!({{
        let type_name = ~{doubles(type_name)};@
        let value = ~{in_expr};@

        if value.trunc()~ != value || value < 0 as f64 || value > ~{type_name}::MAX as f64 ~{{
            return Err({
                platform.error(frag!(
                    format!("cannot cast {value} to {type_name}")
                ))
            });
        }}@

        value as ~{type_name}
    }})
}

fn render_f64_to_char_expr<'db, InExpr>(
    platform: Platform,
    in_expr: InExpr,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
{
    let as_u32 = render_f64_to_unsigned_expr(platform, TypeKind::u32, in_expr);
    let error_expr = platform.error(frag!(format!("cannot cast {value} to char")));

    frag!({{
        let value = {as_u32};@
        char::from_u32(value).ok_or_else(||~{error_expr})?
    }})
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
            unreachable!("borrowed Rust values are not returned through the shim boundary")
        }
        _ => unreachable!("Rust value `{source:?}` cannot be cast to boundary f64"),
    });

    adapt_to_cast_output(expr, output)
}
