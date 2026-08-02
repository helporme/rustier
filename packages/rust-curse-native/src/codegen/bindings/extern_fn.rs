use crate::{
    codegen::{
        api::Api,
        bindings::{
            bir::SourceType,
            cast_graph::{BindingsCastCompiler, Cast},
        },
        db::Db,
        fragment::{FragmentOnce, comma_list, fragment},
        ir::{PassingMode, Type},
    },
    frag,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ExternFn<'db, Params, SourceExpr> {
    pub name: &'db str,
    pub params: Params,
    pub return_type: Option<Type<'db>>,
    pub source_expr: SourceExpr,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ExternFnParam<'db> {
    pub name: &'db str,
    pub type_: Type<'db>,
    pub mode: PassingMode,
    pub is_self: bool,
}

impl<'db> ExternFnParam<'db> {
    /// Source type of the parameter.
    pub fn source_type(&self) -> SourceType<'db> {
        match self.mode {
            PassingMode::Ref => SourceType::Ref {
                is_mutable: false,
                inner: Box::new(self.type_.source_type_owned()),
            },
            PassingMode::Mut => SourceType::Ref {
                is_mutable: true,
                inner: Box::new(self.type_.source_type_owned()),
            },
            PassingMode::Move => self.type_.source_type_owned(),
        }
    }

    pub fn arg_name(&self) -> impl FragmentOnce + use<'db> {
        let name = self.name;
        let is_self = self.is_self;

        fragment(move |p| {
            if is_self {
                "self".apply(p)
            } else {
                frag!(__p__ { name }).apply(p);
            }
        })
    }
}

pub fn compile_extern_fn<'db, CastCompiler, Params, SourceExpr>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    fn_: ExternFn<'db, Params, SourceExpr>,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
    Params: IntoIterator<Item = ExternFnParam<'db>> + 'db,
    Params::IntoIter: Clone + 'db,
    SourceExpr: FragmentOnce + 'db,
{
    let api = Api::get(db);

    let return_type = fn_.return_type;
    let result_type = fragment(move |p| match return_type {
        Some(type_) => api
            .result_type(db, type_.extern_out().fqn_decl(db))
            .apply(p),
        None => api.result_type(db, "()").apply(p),
    });

    let params = fn_.params.into_iter();

    frag!(
        {api.header_renamed(db, fn_.name)}@
        pub fn ~{fn_.name}({compile_extern_fn_params(db, params.clone())})~ -> ~{result_type}~ {{
            {compile_extern_fn_body(db, cast_compiler, params, return_type, fn_.source_expr)}
        }}
    )
}

fn compile_extern_fn_params<'db, Params>(db: &'db dyn Db, params: Params) -> impl FragmentOnce + 'db
where
    Params: Iterator<Item = ExternFnParam<'db>> + 'db,
{
    comma_list(params.map(move |param| {
        fragment(move |p| {
            if param.is_self {
                match param.mode {
                    PassingMode::Ref => frag!(&self).apply(p),
                    PassingMode::Mut => frag!(&mut self).apply(p),
                    PassingMode::Move => frag!(self).apply(p),
                }
            } else {
                frag!(
                    {param.arg_name()}: ~{param.type_.extern_in().fqn_decl(db)}
                )
                .apply(p);
            }
        })
    }))
}

/// Compiles a binding function body for a function returning the active
/// [`crate::codegen::api::Api::result_type`] type, allowing generated `?`
/// expressions.
fn compile_extern_fn_body<'db, CastCompiler, Params, SourceExpr>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    params: Params,
    return_type: Option<Type<'db>>,
    source_expr: SourceExpr,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
    Params: Iterator<Item = ExternFnParam<'db>> + Clone + 'db,
    SourceExpr: FragmentOnce + 'db,
{
    fragment(move |p| {
        let cast_param_to_source_types = params.map(|param| Cast {
            from: param.type_.extern_in(),
            to: param.source_type(),
            in_expr: param.arg_name(),
            out_var_name: param.name,
        });

        match return_type {
            Some(ret_type) => {
                let result_expr = cast_compiler.clone().compile_casts_expr(
                    db,
                    cast_param_to_source_types,
                    source_expr,
                );

                let casted_result_expr = cast_compiler.compile_cast_expr(
                    db,
                    ret_type.source_type_owned(),
                    ret_type.extern_out(),
                    result_expr,
                );

                frag!(Ok({ casted_result_expr })).apply(p);
            }
            None => {
                let fn_call_stmt = cast_compiler.compile_casts_stmt(
                    db,
                    cast_param_to_source_types,
                    frag!({source_expr};),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_rust_snap, codegen::bindings::cast_graph::test::TestCastCompiler};

    #[test]
    fn zero_args_without_return() {
        assert_rust_snap! {
            source: { fn noop() {} },
            expression: |db, p| {
                let params: [ExternFnParam; 0] = [];
                let fn_ = ExternFn {
                    name: "test_fn",
                    params,
                    return_type: None,
                    source_expr: "source()",
                };

                compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
            },
        }
    }

    #[test]
    fn zero_args_with_return() {
        assert_rust_snap! {
            source: { fn noop() {} },
            expression: |db, p| {
                let params: [ExternFnParam; 0] = [];
                let fn_ = ExternFn {
                    name: "test_fn",
                    params,
                    return_type: Some(Type::f64),
                    source_expr: "source()",
                };

                compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
            },
        }
    }

    #[test]
    fn one_arg_without_return() {
        assert_rust_snap! {
            source: { fn noop() {} },
            expression: |db, p| {
                let params = [ExternFnParam {
                    name: "arg",
                    type_: Type::f64,
                    mode: PassingMode::Move,
                    is_self: false,
                }];

                let fn_ = ExternFn {
                    name: "test_fn",
                    params,
                    return_type: None,
                    source_expr: "source(arg)",
                };

                compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
            },
        }
    }

    #[test]
    fn one_arg_with_return() {
        assert_rust_snap! {
            source: { fn noop() {} },
            expression: |db, p| {
                let params = [ExternFnParam {
                    name: "arg",
                    type_: Type::f64,
                    mode: PassingMode::Move,
                    is_self: false,
                }];

                let fn_ = ExternFn {
                    name: "test_fn",
                    params,
                    return_type: Some(Type::f64),
                    source_expr: "source(arg)",
                };

                compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
            },
        }
    }

    #[test]
    fn two_arg_without_return() {
        assert_rust_snap! {
            source: { fn noop() {} },
            expression: |db, p| {
                let params = [ExternFnParam {
                    name: "arg1",
                    type_: Type::f64,
                    mode: PassingMode::Move,
                    is_self: false,
                }, ExternFnParam {
                    name: "arg2",
                    type_: Type::f64,
                    mode: PassingMode::Ref,
                    is_self: false,
                }];

                let fn_ = ExternFn {
                    name: "test_fn",
                    params,
                    return_type: None,
                    source_expr: "source(arg1, arg2)",
                };

                compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
            },
        }
    }

    #[test]
    fn two_arg_with_return() {
        assert_rust_snap! {
            source: { fn noop() {} },
            expression: |db, p| {
                let params = [ExternFnParam {
                    name: "arg1",
                    type_: Type::f64,
                    mode: PassingMode::Move,
                    is_self: false,
                }, ExternFnParam {
                    name: "arg2",
                    type_: Type::f64,
                    mode: PassingMode::Ref,
                    is_self: false,
                }];

                let fn_ = ExternFn {
                    name: "test_fn",
                    params,
                    return_type: Some(Type::f64),
                    source_expr: "source(arg1, arg2)",
                };

                compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
            },
        }
    }

    #[test]
    fn self_arg() {
        assert_rust_snap! {
            source: { fn noop() {} },
            expression: |db, p| {
                let params = [ExternFnParam {
                    name: "arg",
                    type_: Type::f64,
                    mode: PassingMode::Move,
                    is_self: true,
                }];

                let fn_ = ExternFn {
                    name: "test_fn",
                    params,
                    return_type: None,
                    source_expr: "source(arg)",
                };

                compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
            },
        }
    }

    #[test]
    fn self_ref_arg() {
        assert_rust_snap! {
            source: { fn noop() {} },
            expression: |db, p| {
                let params = [ExternFnParam {
                    name: "arg",
                    type_: Type::f64,
                    mode: PassingMode::Ref,
                    is_self: true,
                }];

                let fn_ = ExternFn {
                    name: "test_fn",
                    params,
                    return_type: None,
                    source_expr: "source(arg)",
                };

                compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
            },
        }
    }

    #[test]
    fn self_mut_arg() {
        assert_rust_snap! {
            source: { fn noop() {} },
            expression: |db, p| {
                let params = [ExternFnParam {
                    name: "arg",
                    type_: Type::f64,
                    mode: PassingMode::Mut,
                    is_self: true,
                }];

                let fn_ = ExternFn {
                    name: "test_fn",
                    params,
                    return_type: None,
                    source_expr: "source(arg)",
                };

                compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
            },
        }
    }

    #[test]
    fn self_two_arg() {
        assert_rust_snap! {
            source: { fn noop() {} },
            expression: |db, p| {
                let params = [ExternFnParam {
                    name: "arg",
                    type_: Type::f64,
                    mode: PassingMode::Move,
                    is_self: true,
                }, ExternFnParam {
                    name: "arg2",
                    type_: Type::f64,
                    mode: PassingMode::Ref,
                    is_self: false,
                }];

                let fn_ = ExternFn {
                    name: "test_fn",
                    params,
                    return_type: None,
                    source_expr: "source(arg, arg2)",
                };

                compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
            },
        }
    }
}
