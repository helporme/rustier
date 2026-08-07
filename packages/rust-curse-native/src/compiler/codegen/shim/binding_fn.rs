use crate::{
    compiler::{
        bindings_ir::{BindingFn, BindingFnBody},
        codegen::{
            api::Api,
            bindings::cast_graph::{BindingsCastCompiler, Cast},
            fragment::{FragmentOnce, Printer, comma_list, fragment},
        },
        db::Db,
        ir::{ItemIdExt, Type},
    },
    frag,
};

pub fn render_binding_fn<'db, CastCompiler>(
    db: &'db dyn Db,
    fn_: BindingFn<'db>,
    cast_compiler: CastCompiler,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    BindingFnRenderer {
        fn_,
        cast_compiler,
        db,
    }
}

struct BindingFnRenderer<'db, CastCompiler> {
    fn_: BindingFn<'db>,
    cast_compiler: CastCompiler,
    db: &'db dyn Db,
}

impl<'db, CastCompiler> FragmentOnce for BindingFnRenderer<'db, CastCompiler>
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, CastCompiler> BindingFnRenderer<'db, CastCompiler>
where
    CastCompiler: BindingsCastCompiler<'db> + Clone,
{
    fn render<'a>(&'a self) -> impl FragmentOnce + 'a {
        let api = Api::get(self.db);

        let return_type = self.fn_.external.return_type(self.db);
        let result_type = fragment(move |p| match return_type {
            Some(type_) => api
                .result_type(self.db, type_.render_type_decl(self.db))
                .apply(p),
            None => api.result_type(self.db, "()").apply(p),
        });

        let fn_symbol = self.fn_.external.symbol(self.db).value(self.db);

        frag!(
            {api.header_renamed(self.db, fn_symbol)}@
            pub fn ~{fn_symbol}({self.render_params()})~ -> ~{result_type}~ {{
                {self.render_body()}
            }}
        )
    }

    fn render_params<'a>(&'a self) -> impl FragmentOnce + 'a {
        comma_list(self.fn_.params.iter().enumerate().map(move |(i, param)| {
            fragment(move |p| {
                if param.external.is_self(self.db) {
                    match param.source_type {
                        Type::Value(_) => frag!(self).apply(p),
                        Type::Borrowed {
                            is_mutable: false, ..
                        } => frag!(&self).apply(p),
                        Type::Borrowed {
                            is_mutable: true, ..
                        } => frag!(&mut self).apply(p),
                    }
                } else {
                    frag!(
                        _{i}: ~{param.external.type_(self.db).render_type_decl(self.db)}
                    )
                    .apply(p);
                }
            })
        }))
    }

    fn render_body<'a>(&'a self) -> impl FragmentOnce + 'a {
        fragment(|p| {
            let cast_param_to_source_types =
                self.fn_.params.iter().enumerate().map(|(i, param)| Cast {
                    from: param.external.type_(self.db),
                    to: param.source_type,
                    in_expr: frag!(_{i}),
                    out_var_name: frag!(_{i}),
                });

            match (
                self.fn_.src_ret_type,
                self.fn_.external.return_type(self.db),
            ) {
                (Some(source_ret_type), Some(extern_ret_type)) => {
                    let result_expr = self.cast_compiler.clone().compile_casts_expr(
                        self.db,
                        cast_param_to_source_types,
                        self.render_source_expr(),
                    );

                    let casted_result_expr = self.cast_compiler.compile_cast_expr(
                        self.db,
                        source_ret_type,
                        extern_ret_type,
                        result_expr,
                    );

                    frag!(Ok({ casted_result_expr })).apply(p);
                }
                _ => {
                    let fn_call_stmt = self.cast_compiler.compile_casts_stmt(
                        self.db,
                        cast_param_to_source_types,
                        frag!({self.render_source_expr()};),
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

    fn render_source_expr<'a>(&'a self) -> impl FragmentOnce + 'a {
        let params = comma_list(self.fn_.params.iter().enumerate().map(|(i, _)| frag!(_{i})));
        match self.fn_.body {
            BindingFnBody::FnCall(fn_) => frag!({ fn_.fqn(self.db) }({ params })),
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::{
//         assert_rust_snap,
// compiler::codegen::bindings::cast_graph::test::TestCastCompiler,     };

//     #[test]
//     fn zero_args_without_return() {
//         assert_rust_snap! {
//             source: { fn noop() {} },
//             expression: |db, p| {
//                 let params: [ExternFnParam; 0] = [];
//                 let fn_ = ExternFn {
//                     name: "test_fn",
//                     params,
//                     return_type: None,
//                     source_expr: "source()",
//                 };

//                 compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
//             },
//         }
//     }

//     #[test]
//     fn zero_args_with_return() {
//         assert_rust_snap! {
//             source: { fn noop() {} },
//             expression: |db, p| {
//                 let params: [ExternFnParam; 0] = [];
//                 let fn_ = ExternFn {
//                     name: "test_fn",
//                     params,
//                     return_type: Some(ValueType::f64),
//                     source_expr: "source()",
//                 };

//                 compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
//             },
//         }
//     }

//     #[test]
//     fn one_arg_without_return() {
//         assert_rust_snap! {
//             source: { fn noop() {} },
//             expression: |db, p| {
//                 let params = [ExternFnParam {
//                     name: "arg",
//                     type_: Type::Value(ValueType::f64),
//                     is_self: false,
//                 }];

//                 let fn_ = ExternFn {
//                     name: "test_fn",
//                     params,
//                     return_type: None,
//                     source_expr: "source(arg)",
//                 };

//                 compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
//             },
//         }
//     }

//     #[test]
//     fn one_arg_with_return() {
//         assert_rust_snap! {
//             source: { fn noop() {} },
//             expression: |db, p| {
//                 let params = [ExternFnParam {
//                     name: "arg",
//                     type_: Type::Value(ValueType::f64),
//                     is_self: false,
//                 }];

//                 let fn_ = ExternFn {
//                     name: "test_fn",
//                     params,
//                     return_type: Some(ValueType::f64),
//                     source_expr: "source(arg)",
//                 };

//                 compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
//             },
//         }
//     }

//     #[test]
//     fn two_arg_without_return() {
//         assert_rust_snap! {
//             source: { fn noop() {} },
//             expression: |db, p| {
//                 let params = [ExternFnParam {
//                     name: "arg1",
//                     type_: Type::Value(ValueType::f64),
//                     is_self: false,
//                 }, ExternFnParam {
//                     name: "arg2",
//                     type_: Type::Borrowed {
//                         is_mutable: false,
//                         inner: ValueType::f64,
//                     },
//                     is_self: false,
//                 }];

//                 let fn_ = ExternFn {
//                     name: "test_fn",
//                     params,
//                     return_type: None,
//                     source_expr: "source(arg1, arg2)",
//                 };

//                 compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
//             },
//         }
//     }

//     #[test]
//     fn two_arg_with_return() {
//         assert_rust_snap! {
//             source: { fn noop() {} },
//             expression: |db, p| {
//                 let params = [ExternFnParam {
//                     name: "arg1",
//                     type_: Type::Value(ValueType::f64),
//                     is_self: false,
//                 }, ExternFnParam {
//                     name: "arg2",
//                     type_: Type::Borrowed {
//                         is_mutable: false,
//                         inner: ValueType::f64,
//                     },
//                     is_self: false,
//                 }];

//                 let fn_ = ExternFn {
//                     name: "test_fn",
//                     params,
//                     return_type: Some(ValueType::f64),
//                     source_expr: "source(arg1, arg2)",
//                 };

//                 compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
//             },
//         }
//     }

//     #[test]
//     fn self_arg() {
//         assert_rust_snap! {
//             source: { fn noop() {} },
//             expression: |db, p| {
//                 let params = [ExternFnParam {
//                     name: "arg",
//                     type_: Type::Value(ValueType::f64),
//                     is_self: true,
//                 }];

//                 let fn_ = ExternFn {
//                     name: "test_fn",
//                     params,
//                     return_type: None,
//                     source_expr: "source(arg)",
//                 };

//                 compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
//             },
//         }
//     }

//     #[test]
//     fn self_ref_arg() {
//         assert_rust_snap! {
//             source: { fn noop() {} },
//             expression: |db, p| {
//                 let params = [ExternFnParam {
//                     name: "arg",
//                     type_: Type::Borrowed {
//                         is_mutable: false,
//                         inner: ValueType::f64,
//                     },
//                     is_self: true,
//                 }];

//                 let fn_ = ExternFn {
//                     name: "test_fn",
//                     params,
//                     return_type: None,
//                     source_expr: "source(arg)",
//                 };

//                 compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
//             },
//         }
//     }

//     #[test]
//     fn self_mut_arg() {
//         assert_rust_snap! {
//             source: { fn noop() {} },
//             expression: |db, p| {
//                 let params = [ExternFnParam {
//                     name: "arg",
//                     type_: Type::Borrowed {
//                         is_mutable: true,
//                         inner: ValueType::f64,
//                     },
//                     is_self: true,
//                 }];

//                 let fn_ = ExternFn {
//                     name: "test_fn",
//                     params,
//                     return_type: None,
//                     source_expr: "source(arg)",
//                 };

//                 compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
//             },
//         }
//     }

//     #[test]
//     fn self_two_arg() {
//         assert_rust_snap! {
//             source: { fn noop() {} },
//             expression: |db, p| {
//                 let params = [ExternFnParam {
//                     name: "arg",
//                     type_: Type::Value(ValueType::f64),
//                     is_self: true,
//                 }, ExternFnParam {
//                     name: "arg2",
//                     type_: Type::Borrowed {
//                         is_mutable: false,
//                         inner: ValueType::f64,
//                     },
//                     is_self: false,
//                 }];

//                 let fn_ = ExternFn {
//                     name: "test_fn",
//                     params,
//                     return_type: None,
//                     source_expr: "source(arg, arg2)",
//                 };

//                 compile_extern_fn(db, TestCastCompiler, fn_).apply(p);
//             },
//         }
//     }
// }
