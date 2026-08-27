use crate::{
    compiler::{
        codegen::{
            fragment::{FragmentOnce, FragmentPrint, Printer, comma_list, frag, fragment},
            js::cast::{Cast, CastRenderer, Renderer},
        },
        db::Db,
        ir::facade::FacadeFunction,
    },
};

#[salsa::tracked]
impl<'db> FacadeFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db) -> String {
        self.render_with(db, Renderer).print()
    }

    fn render_with<Casts>(self, db: &'db dyn Db, casts: Casts) -> impl FragmentOnce
    where
        Casts: CastRenderer<'db> + Copy + Clone + 'db,
    {
        FacadeFunctionRenderer {
            function: self,
            db,
            casts,
        }
    }
}

#[derive(Copy, Clone)]
struct FacadeFunctionRenderer<'db, Casts> {
    function: FacadeFunction<'db>,
    db: &'db dyn Db,
    casts: Casts,
}

impl<'db, Casts> FragmentOnce for FacadeFunctionRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, Casts> FacadeFunctionRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn render(self) -> impl FragmentOnce {
        let name = self.function.name(self.db);
        let ffi_ret_type = self.function.ffi(self.db).ret_type(self.db);
        let facade_ret_type = self.function.ret_type(self.db);

        let body = fragment(move |p| match (ffi_ret_type, facade_ret_type) {
            (Some(ffi_ret_type), Some(facade_ret_type)) => {
                self.render_param_casts({
                    self.casts.render_stmt(
                        self.db,
                        Cast {
                            from: ffi_ret_type,
                            to: facade_ret_type,
                            in_expr: self.render_call_expr(),
                            out_var_name: "result",
                        },
                        frag!(return result;),
                    )
                })
                .apply(p);
            }
            (None, None) => {
                let call_stmt = frag!({ self.render_call_expr()};);
                self.render_param_casts(call_stmt).apply(p);
            }
            _ => unreachable!(),
        });

        frag!(
            export function~{name}~({self.render_params()})~{{
                {body}@
            }}
        )
    }

    fn render_params(self) -> impl FragmentOnce {
        comma_list(
            self.function
                .params(self.db)
                .iter()
                .enumerate()
                .map(|(i, _)| Self::param_name(i)),
        )
    }

    fn render_param_casts(self, continuation: impl FragmentOnce + 'db) -> impl FragmentOnce + 'db {
        let ffi_params = self.function.ffi(self.db).params(self.db);
        let param_casts = self
            .function
            .params(self.db)
            .iter()
            .zip(ffi_params)
            .enumerate()
            .map(|(i, (param, ffi_param))| Cast {
                from: param.type_(self.db),
                to: ffi_param.type_(self.db),
                in_expr: Self::param_name(i),
                out_var_name: Self::casted_param_name(i),
            });

        self.casts.render_many(self.db, param_casts, continuation)
    }

    fn render_call_expr(self) -> impl FragmentOnce {
        let ffi_fn_symbol = self.function.ffi(self.db).symbol(self.db);
        let params = comma_list(
            self.function
                .params(self.db)
                .iter()
                .enumerate()
                .map(|(i, _)| Self::casted_param_name(i)),
        );

        frag!({ ffi_fn_symbol }({ params }))
    }

    fn param_name(i: usize) -> impl FragmentOnce {
        frag!(_{i})
    }

    fn casted_param_name(i: usize) -> impl FragmentOnce {
        frag!(__ { i })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_js_snap,
        compiler::{
            codegen::{fragment::FragmentOnce as _, js::cast::test::TestCastRenderer},
            ir::rust::Rust,
        },
    };

    macro_rules! test_function {
        ($name:ident, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_js_snap! {
                    source: { $($source)* },
                    expression: |db, printer| {
                        let db = &*db;
                        let root = Rust::get(db)
                            .crates(db)
                            .first()
                            .expect("test crate should be lowered")
                            .root(db)
                            .shim(db)
                            .facade(db);
                        let [function] = root.fns(db).as_slice() else {
                            panic!("test crate root should contain exactly one facade function");
                        };

                        function
                            .render_with(db, TestCastRenderer)
                            .apply(printer);
                    },
                }
            }
        };
    }

    test_function!(zero_args_without_return, {
        pub fn test_fn() {}
    });

    test_function!(zero_args_with_return, {
        pub fn test_fn() -> i32 {
            0
        }
    });

    test_function!(one_arg_without_return, {
        pub fn test_fn(_value: i32) {}
    });

    test_function!(one_arg_with_return, {
        pub fn test_fn(value: i32) -> i32 {
            value
        }
    });

    test_function!(two_args_without_return, {
        pub fn test_fn(_direct: i32, _borrowed: &i32) {}
    });

    test_function!(two_args_with_return, {
        pub fn test_fn(direct: i32, borrowed: &i32) -> i32 {
            direct + borrowed
        }
    });

    test_function!(class_argument_and_return, {
        #[derive(Clone)]
        pub struct Test;

        pub fn test_fn(value: Test) -> Test {
            value
        }
    });
}
