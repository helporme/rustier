use crate::{
    compiler::{
        codegen::{
            fragment::{FragmentOnce, fragment},
            shim::tethering::TetheringStructType,
        },
        db::Db,
        ir::{rust::Type, shim::ShimType},
    },
    frag,
};

use super::{
    CastOutput, adapt_to_cast_output,
    tethering::{render_tethered_to_type, render_type_to_tethered_expr},
};

pub fn render_class_to_tethered<'db, InExpr, OutVar, Continue>(
    class: ShimType<'db>,
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
    class: ShimType<'db>,
    in_expr: InExpr,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
{
    let ShimType::Class { is_borrowed, .. } = class else {
        unreachable!("class-to-tethered cast requires a class source");
    };

    fragment(move |printer| {
        if is_borrowed {
            frag!(std::rc::Rc::clone(&{ in_expr }.src)).apply(printer);
        } else {
            frag!({ in_expr }.src).apply(printer);
        }
    })
}

pub fn render_tethered_to_class<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    class: ShimType<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let ShimType::Class { is_borrowed, inner } = class else {
        unreachable!("tethered-to-class cast requires a class target");
    };
    assert!(
        !is_borrowed,
        "a newly rendered class wrapper is always an owned shim value",
    );

    adapt_to_cast_output(frag!({inner.symbol(db)}~{{ src: {in_expr} }}), output)
}

pub fn render_class_to_type<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    class: ShimType<'db>,
    target: Type<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let ShimType::Class { inner, .. } = class else {
        unreachable!("class-to-type cast requires a class source");
    };
    let tethered = TetheringStructType {
        source_type: inner.src(db).into(),
    };
    let tethered_expr = render_class_to_tethered_expr(class, in_expr);

    render_tethered_to_type(db, tethered, target, tethered_expr, output)
}

pub fn render_type_to_class<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    source: Type<'db>,
    class: ShimType<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let ShimType::Class { is_borrowed, inner } = class else {
        unreachable!("type-to-class cast requires a class target");
    };
    assert!(
        !is_borrowed,
        "a source value can only be cast to an owned class wrapper",
    );
    let tethered = TetheringStructType {
        source_type: inner.src(db).into(),
    };
    let tethered_expr = render_type_to_tethered_expr(db, source, tethered, in_expr);
    let class_expr = frag!({inner.symbol(db)}~{{ src: {tethered_expr} }});

    adapt_to_cast_output(class_expr, output)
}
