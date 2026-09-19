use crate::compiler::{
    codegen::fragment::{FragmentOnce, frag, fragment},
    db::Db,
    ir::rust::Type,
};

use super::{CastOutput, adapt_to_cast_output, borrowing::render_owned_to_borrowed};

pub fn render_string_to_type<'db, InExpr, OutVar, Continue>(
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
    fragment(move |printer| match target {
        Type::String => {
            adapt_to_cast_output(in_expr, output).apply(printer);
        }
        Type::Borrowed(borrowed) if borrowed.inner(db) == Type::String => {
            render_owned_to_borrowed(db, target, in_expr, output).apply(printer);
        }
        Type::Borrowed(borrowed) if borrowed.inner(db) == Type::Str => match output {
            CastOutput::Expression => {
                unreachable!(
                    "a str borrowed from a cast-owned String cannot escape as an expression"
                )
            }
            CastOutput::Continue {
                out_var_name,
                continue_with,
            } => {
                let borrow = if borrowed.is_mutable(db) {
                    frag!(__cast_owned_value.as_mut_str())
                } else {
                    frag!(__cast_owned_value.as_str())
                };

                frag!({{
                    let mut __cast_owned_value = {in_expr};
                    let ~{out_var_name}~ = ~{borrow};
                    {continue_with}
                }})
                .apply(printer);
            }
        },
        _ => unreachable!("boundary String cannot be cast to `{target:?}`"),
    })
}
