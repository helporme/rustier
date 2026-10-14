use crate::compiler::{
    codegen::fragment::{FragmentOnce, doubles, frag, fragment},
    db::Db,
    ir::facade::FacadeClass,
};

pub fn render_facade_class_to_abi<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    source: FacadeClass<'db>,
    in_expr: InExpr,
    out_var_name: OutVar,
    continue_with: Continue,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    let expected_class = source.import_alias(db);

    let input = fragment(move |p| match source.coerce(db) {
        Some(coerce) => frag!({expected_class}.{coerce.name(db)}({in_expr})).apply(p),
        None => in_expr.apply(p),
    });

    let error_message = frag!(
        {doubles((
            r#"Expected a value of type \""#,
            source.name(db),
            r#"\", but value of type \""#,
        ))}~ + typeof __cast_input + ~{doubles(r#"\" was passed: \""#)}~ + String(__cast_input) ~+~ {doubles(r#"\""#)}
    );

    frag!({{
        const __cast_input~ = ~{input};@
        if (!(__cast_input~ instanceof ~{expected_class}))~{{
            throw new TypeError(
                {error_message}
            );
        }}@
        const ~{out_var_name}~ = __cast_input.$handle();@
        {continue_with}
    }})
}

pub fn render_abi_handle_to_facade<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    target: FacadeClass<'db>,
    in_expr: InExpr,
    out_var_name: OutVar,
    continue_with: Continue,
) -> impl FragmentOnce + 'db
where
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    frag!({{
        const __cast_input~ = ~{in_expr};@
        const ~{out_var_name}~ = ~{target.import_alias(db)}.$from_handle(__cast_input);@
        {continue_with}
    }})
}
