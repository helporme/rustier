use crate::{
    compiler::{
        codegen::fragment::{FragmentOnce, doubles},
        db::Db,
        ir::{facade::FacadeClass, ffi::FfiClass},
    },
    frag,
};

pub fn render_facade_class_to_ffi<'db, InExpr, OutVar, Continue>(
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
    let expected_class = source.name(db);
    let error_message = frag!(
        {doubles((
            r#"Expected a value of type \""#,
            expected_class,
            r#"\", but value of type \""#,
        ))}~ + typeof __cast_input + ~{doubles(r#"\" was passed: \""#)}~ + String(__cast_input) ~+~ {doubles(r#"\""#)}
    );

    frag!({{
        const __cast_input~ = ~{in_expr};@
        if (!(__cast_input~ instanceof ~{expected_class}))~{{
            throw new TypeError(
                {error_message}
            );
        }}@
        const ~{out_var_name}~ = __cast_input.$ffi();@
        {continue_with}
    }})
}

pub fn render_ffi_class_to_facade<'db, InExpr, OutVar, Continue>(
    db: &'db dyn Db,
    source: FfiClass<'db>,
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
    let expected_class = source.symbol(db);
    let error_message = frag!(
        {doubles((
            r#"Expected a value of type \""#,
            expected_class,
            r#"\", but value of type \""#,
        ))}~ + typeof __cast_input + ~{doubles(r#"\" was passed: \""#)}~ + String(__cast_input) ~+~ {doubles(r#"\""#)}
    );

    frag!({{
        const __cast_input~ = ~{in_expr};@
        if (!(__cast_input~ instanceof ~{expected_class}))~{{
            throw new TypeError(
                {error_message}
            );
        }}@
        const ~{out_var_name}~ = ~{target.name(db)}.$from_ffi(__cast_input);@
        {continue_with}
    }})
}
