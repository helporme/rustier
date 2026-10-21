use crate::compiler::{
    codegen::fragment::{FragmentOnce, frag},
    db::Db,
    ir::{runtime::tethering::RuntimeTethering, rust::Type},
};

use super::{
    CastOutput, adapt_to_cast_output,
    tethering::{render_tethered_to_type, render_type_to_tethered_expr},
};

pub fn render_handle_to_tethered<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    target: RuntimeTethering<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let type_name = target.tethering_struct_name(db);
    let value = frag!(unsafe {
        { rustier_ext::runtime::rust_handles::clone_from_handle::<{ type_name }>({ in_expr }) }
    });

    adapt_to_cast_output(value, output)
}

pub fn render_tethered_to_handle<'db, InExpr, OutVar, Continue>(
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    adapt_to_cast_output(
        frag!(rustier_ext::runtime::rust_handles::into_handle({ in_expr })),
        output,
    )
}

pub fn render_handle_to_type<'db, InExpr, OutVar, Continue>(
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
    let tethered = target
        .tethering(db)
        .expect("handle casts target structs or enums");
    let value = render_handle_to_tethered(db, tethered, in_expr, CastOutput::<(), ()>::Expression);

    render_tethered_to_type(db, tethered, target, value, output)
}

pub fn render_type_to_handle<'db, InExpr, OutVar, Continue>(
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
    let tethered = source
        .tethering(db)
        .expect("handle casts source structs or enums");
    let value = render_type_to_tethered_expr(db, source, tethered, in_expr);

    render_tethered_to_handle(value, output)
}
