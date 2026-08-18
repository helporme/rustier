use crate::{
    compiler::{
        codegen::fragment::{FragmentOnce, fragment},
        ir::rust::Type,
    },
    frag,
};

use super::CastOutput;

/// Renders a cast statement from owned T to borrowed &T / &mut T
pub fn render_owned_to_borrowed<'db, InExpr, OutVar, Continue>(
    target: Type<'db>,
    in_expr: InExpr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    fragment(move |printer| {
        let Type::Borrowed { is_mutable, .. } = target else {
            unreachable!();
        };

        match output {
            CastOutput::Expression => {
                panic!("a borrow of a cast-owned value cannot escape as an expression")
            }
            CastOutput::Continue {
                out_var_name,
                continue_with,
            } => {
                let borrow = if is_mutable {
                    frag!(&mut __cast_owned_value)
                } else {
                    frag!(&__cast_owned_value)
                };

                frag!({{
                    let mut __cast_owned_value = {in_expr};
                    let ~{out_var_name}~ = ~{borrow};
                    {continue_with}
                }})
                .apply(printer);
            }
        }
    })
}
