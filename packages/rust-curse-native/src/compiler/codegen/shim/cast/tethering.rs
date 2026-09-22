use crate::compiler::{
    codegen::{
        fragment::{FragmentOnce, frag, fragment},
        shim::tethering::TetheringStructType,
    },
    db::Db,
    ir::rust::{Type, TypeKind},
};

use super::{CastOutput, adapt_to_cast_output};

pub fn render_type_to_tethered<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    source: Type<'db>,
    target: TetheringStructType<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let expr = render_type_to_tethered_expr(db, source, target, in_expr);
    adapt_to_cast_output(expr, output)
}

pub fn render_type_to_tethered_expr<'db, InExpr>(
    db: &'db dyn Db,
    source: Type<'db>,
    target: TetheringStructType<'db>,
    in_expr: InExpr,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
{
    assert_eq!(
        source.kind(db),
        target.source_type.kind(db),
        "source and tethering cast types must refer to the same value type",
    );
    let tethering_struct_name = target.source_type.tethering_struct_name(db);

    // TODO: remove Rc once class ownership no longer requires it.
    frag!(std::rc::Rc::new({tethering_struct_name}::new_owned({in_expr})))
}

pub fn render_tethered_to_type<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    source: TetheringStructType<'db>,
    target: Type<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    assert_eq!(
        source.source_type.kind(db),
        target.deborrow(db).kind(db),
        "tethering and target cast types must refer to the same value type",
    );

    fragment(move |printer| match target.kind(db) {
        TypeKind::Borrowed(borrowed) => match output {
            CastOutput::Expression => {
                unreachable!(
                    "a value borrowed through tethering access cannot escape as an expression"
                )
            }
            CastOutput::Continue {
                out_var_name,
                continue_with,
            } => {
                if borrowed.is_mutable(db) {
                    frag!(
                        {in_expr}.access_mut(|value| Ok({{
                            let ~{out_var_name}~ = value;
                            {continue_with}
                        }}))?
                    )
                    .apply(printer);
                } else {
                    frag!(
                        {in_expr}.access(|value| Ok({{
                            let ~{out_var_name}~ = value;
                            {continue_with}
                        }}))?
                    )
                    .apply(printer);
                }
            }
        },
        _ => {
            let expr = frag!({ in_expr }.access(|value| Ok(value.clone()))?);
            adapt_to_cast_output(expr, output).apply(printer);
        }
    })
}
