use crate::{
    compiler::codegen::fragment::{FragmentOnce, doubles, frag},
};

pub fn render_string_cast<'db, InExpr, OutVar, Continue>(
    in_expr: InExpr,
    out_var_name: OutVar,
    continue_with: Continue,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    render_checked_identity("string", in_expr, out_var_name, continue_with)
}

pub fn render_bool_cast<'db, InExpr, OutVar, Continue>(
    in_expr: InExpr,
    out_var_name: OutVar,
    continue_with: Continue,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    render_checked_identity("boolean", in_expr, out_var_name, continue_with)
}

pub fn render_number_cast<'db, InExpr, OutVar, Continue>(
    in_expr: InExpr,
    out_var_name: OutVar,
    continue_with: Continue,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    render_checked_identity("number", in_expr, out_var_name, continue_with)
}

fn render_checked_identity<'db, InExpr, OutVar, Continue>(
    expected_type: &'static str,
    in_expr: InExpr,
    out_var_name: OutVar,
    continue_with: Continue,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let message_start = doubles((
        r#"Expected a value of type \""#,
        expected_type,
        r#"\", but value of type \""#,
    ));

    frag!({{
        const __cast_input~ = ~{in_expr};@
        if (typeof __cast_input~ !== ~{doubles(expected_type)})~{{
            throw new TypeError(
                {message_start}~ + typeof __cast_input + ~{doubles(r#"\" was passed: \""#)}~ + String(__cast_input) ~+~ {doubles(r#"\""#)}
            );
        }}@
        const ~{out_var_name}~ = ~__cast_input;@
        {continue_with}
    }})
}
