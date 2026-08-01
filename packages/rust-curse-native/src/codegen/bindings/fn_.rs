//! Module for compiling standalone (not associated) functions into a binding
//! functions that can be exposed to JavaScript.

use crate::codegen::{
    api::Api,
    bindings::{
        bir::BirSourceType,
        cast_graph::{BindingsCast, BindingsCastCompiler, Cast},
    },
    db::Db,
    fragment::{FragmentOnce, FragmentPrint, comma_list, frag, fragment},
    ir::{
        fn_::{Fn, FnParam, PassingMode},
        item::ItemIdExt as _,
    },
};

#[salsa::tracked]
impl<'db> FnParam<'db> {
    /// Returns the binding name of the parameter, e.g. `__p__foo`.
    #[salsa::tracked(returns(ref))]
    pub fn binding_name(self, db: &'db dyn Db) -> String {
        format!("__p__{}", self.name(db))
    }

    /// Source type of the parameter.
    pub fn source_type(self, db: &'db dyn Db) -> BirSourceType<'db> {
        match self.mode(db) {
            PassingMode::Ref => BirSourceType::Ref {
                is_mutable: false,
                inner: Box::new(self.type_(db).source_type_owned()),
            },
            PassingMode::Mut => BirSourceType::Ref {
                is_mutable: true,
                inner: Box::new(self.type_(db).source_type_owned()),
            },
            PassingMode::Move => self.type_(db).source_type_owned(),
        }
    }
}

/// Compiles a function into a binding function.
#[salsa::tracked(returns(ref))]
pub fn compile_binding_fn<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> String {
    compile_binding_fn_with(db, BindingsCast::Graph, fn_)
}

fn compile_binding_fn_with<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    fn_: Fn<'db>,
) -> String
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    let api = Api::get(db);
    let result_type = fragment(|p| match fn_.return_type(db) {
        Some(type_) => api
            .result_type(db, type_.boundary_out().fqn_decl(db))
            .apply(p),
        None => api.result_type(db, "()").apply(p),
    });

    frag!(
        {api.fn_header(db, fn_)}@
        pub fn ~{fn_.extern_name(db)}({compile_binding_fn_params(db, fn_)})~ -> ~{result_type}~ {{
            {compile_binding_fn_body(db, cast_compiler, fn_)}
        }}
    )
    .print()
}

fn compile_binding_fn_params<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> impl FragmentOnce + 'db {
    frag!({
        comma_list(fn_.params(db).iter().map(|param| {
            frag!(
                {param.binding_name(db)}:~
                {param.type_(db).boundary_in().fqn_decl(db)}
            )
        }))
    })
}

/// Compiles a binding function body for a function returning the active
/// [`crate::codegen::api::Api::result_type`] type, allowing generated `?`
/// expressions.
fn compile_binding_fn_body<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    fn_: Fn<'db>,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    fragment(move |p| {
        let cast_param_to_source_types = fn_.params(db).iter().map(|param| Cast {
            from: param.type_(db).boundary_in(),
            to: param.source_type(db),
            in_expr: param.binding_name(db),
            out_var_name: param.binding_name(db),
        });

        match fn_.return_type(db) {
            Some(ret_type) => {
                let result_expr = cast_compiler.clone().compile_casts_expr(
                    db,
                    cast_param_to_source_types,
                    compile_src_fn_call_expr(db, fn_),
                );

                let casted_result_expr = cast_compiler.compile_cast_expr(
                    db,
                    ret_type.source_type_owned(),
                    ret_type.boundary_out(),
                    result_expr,
                );

                frag!(Ok({ casted_result_expr })).apply(p);
            }
            None => {
                let fn_call_stmt = cast_compiler.compile_casts_stmt(
                    db,
                    cast_param_to_source_types,
                    frag!({compile_src_fn_call_expr(db, fn_)};),
                );

                frag!(
                    {fn_call_stmt}@
                    Ok(())
                )
                .apply(p);
            }
        }
    })
}

/// Compiles a function into a call statement of the function.
///
/// Assumes the binding parameter variables present in the scope.
fn compile_src_fn_call_expr<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> impl FragmentOnce + 'db {
    let params = fragment(move |printer| {
        if fn_.params(db).is_empty() {
            "()".apply(printer);
        } else {
            frag!(({ comma_list(fn_.params(db).iter().map(|param| param.binding_name(db))) }))
                .apply(printer);
        }
    });

    frag!({fn_.fqn(db)}{params})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_rust_snap,
        codegen::{bindings::cast_graph::test::TestCastCompiler, rustdoc::fn_::lower_fn},
    };

    macro_rules! test_binding_fn {
        ($name:ident, $($source:tt)*) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let fn_ = lower_fn(db, db.rustdoc_id("test_fn")).unwrap();
                        compile_binding_fn_with(db, TestCastCompiler, fn_).apply(p);
                    },
                }
            }
        };
    }

    test_binding_fn!(zero_args_without_return, pub fn test_fn() {});

    test_binding_fn!(
        zero_args_with_return,
        pub fn test_fn() -> i32 {
            0
        }
    );

    test_binding_fn!(one_arg_without_return, pub fn test_fn(_value: i32) {});

    test_binding_fn!(
        one_arg_with_return,
        pub fn test_fn(value: i32) -> i32 {
            value
        }
    );

    test_binding_fn!(
        two_args_without_return,
        pub fn test_fn(_direct: i32, _non_direct: &i32) {}
    );

    test_binding_fn!(
        two_args_with_return,
        pub fn test_fn(direct: i32, non_direct: &i32) -> i32 {
            direct + non_direct
        }
    );
}
