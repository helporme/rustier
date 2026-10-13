use crate::compiler::{
    codegen::fragment::{FragmentOnce, frag, fragment},
    db::Db,
    ir::{abi::AbiType, rust::Type},
};

use super::{
    CastOutput, adapt_to_cast_output,
    tethering::{render_tethered_to_type, render_type_to_tethered_expr},
};

pub fn render_class_to_tethered<'db, InExpr, OutVar, Continue>(
    class: AbiType<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let expr = render_class_to_tethered_expr(class, in_expr);
    adapt_to_cast_output(expr, output)
}

fn render_class_to_tethered_expr<'db, InExpr>(
    class: AbiType<'db>,
    in_expr: InExpr,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
{
    let AbiType::Class { borrowed, .. } = class else {
        unreachable!("class-to-tethered cast requires a class source");
    };

    fragment(move |printer| {
        if borrowed {
            frag!({ in_expr }.handle.clone_rc()).apply(printer);
        } else {
            frag!({ in_expr }.handle.into_rc()).apply(printer);
        }
    })
}

pub fn render_tethered_to_class<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    class: AbiType<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let AbiType::Class { class: inner, .. } = class else {
        unreachable!("tethered-to-class cast requires a class target");
    };

    adapt_to_cast_output(
        frag!({inner.symbol(db)}~{{ handle: ({in_expr}).into() }}),
        output,
    )
}

pub fn render_class_to_type<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    class: AbiType<'db>,
    target: Type<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let AbiType::Class { class: inner, .. } = class else {
        unreachable!("class-to-type cast requires a class source");
    };
    let tethered = inner.facade(db).tethering(db);
    let tethered_expr = render_class_to_tethered_expr(class, in_expr);

    render_tethered_to_type(db, tethered, target, tethered_expr, output)
}

pub fn render_type_to_class<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    source: Type<'db>,
    class: AbiType<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let AbiType::Class { class: inner, .. } = class else {
        unreachable!("type-to-class cast requires a class target");
    };
    let tethered = inner.facade(db).tethering(db);
    let tethered_expr = render_type_to_tethered_expr(db, source, tethered, in_expr);

    render_tethered_to_class(db, class, tethered_expr, output)
}
