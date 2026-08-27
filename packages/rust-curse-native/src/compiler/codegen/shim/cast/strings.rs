use crate::{
    compiler::{
        codegen::fragment::{FragmentOnce, frag, fragment},
        ir::rust::{Type, ValueType},
    },
};

use super::{CastOutput, adapt_to_cast_output, borrowing::render_owned_to_borrowed};

pub fn render_string_to_type<'db, InExpr, OutVar, Continue>(
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
        Type::Value(ValueType::String) => {
            adapt_to_cast_output(in_expr, output).apply(printer);
        }
        Type::Borrowed {
            inner: ValueType::String,
            ..
        } => {
            render_owned_to_borrowed(target, in_expr, output).apply(printer);
        }
        Type::Borrowed {
            is_mutable,
            inner: ValueType::Str,
        } => match output {
            CastOutput::Expression => {
                unreachable!(
                    "a str borrowed from a cast-owned String cannot escape as an expression"
                )
            }
            CastOutput::Continue {
                out_var_name,
                continue_with,
            } => {
                let borrow = if is_mutable {
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
