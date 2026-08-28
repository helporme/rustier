use crate::compiler::{
    codegen::{
        fragment::{
            FragmentOnce, FragmentPrint, Printer, comma_list, doubles, frag, fragment, squares,
        },
        js::cast::{Cast, CastRenderer, Renderer},
    },
    db::Db,
    ir::facade::{FacadeFreeFunction, FacadeFunction, FacadeMethod},
};

#[salsa::tracked]
impl<'db> FacadeFreeFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db) -> String {
        self.render_with(db, Renderer).print()
    }

    pub(super) fn render_with<Casts>(self, db: &'db dyn Db, casts: Casts) -> impl FragmentOnce
    where
        Casts: CastRenderer<'db> + Copy + Clone + 'db,
    {
        FreeFunctionRenderer {
            function: self,
            db,
            casts,
        }
    }
}

impl<'db> FacadeMethod<'db> {
    pub(super) fn render_with<Casts>(self, db: &'db dyn Db, casts: Casts) -> impl FragmentOnce
    where
        Casts: CastRenderer<'db> + Copy + Clone + 'db,
    {
        MethodRenderer {
            method: self,
            db,
            casts,
        }
    }

    pub(super) fn render_as_constructor_with<Casts>(
        self,
        db: &'db dyn Db,
        casts: Casts,
    ) -> impl FragmentOnce
    where
        Casts: CastRenderer<'db> + Copy + Clone + 'db,
    {
        ConstructorRenderer {
            method: self,
            db,
            casts,
        }
    }
}

#[derive(Copy, Clone)]
struct FreeFunctionRenderer<'db, Casts> {
    function: FacadeFreeFunction<'db>,
    db: &'db dyn Db,
    casts: Casts,
}

impl<'db, Casts> FragmentOnce for FreeFunctionRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, Casts> FreeFunctionRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn render(self) -> impl FragmentOnce + 'db {
        let name = self.function.name(self.db);

        frag!(
            export function~{name}~({self.render_params()})~{{
                {self.render_body()}@
            }}
        )
    }
}

impl<'db, Casts> FunctionRenderer<'db, Casts> for FreeFunctionRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn db(self) -> &'db dyn Db {
        self.db
    }

    fn function(self) -> FacadeFunction<'db> {
        self.function.as_fn()
    }

    fn casts(self) -> Casts {
        self.casts
    }
}

#[derive(Copy, Clone)]
struct MethodRenderer<'db, Casts> {
    method: FacadeMethod<'db>,
    db: &'db dyn Db,
    casts: Casts,
}

impl<'db, Casts> FragmentOnce for MethodRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, Casts> MethodRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn render(self) -> impl FragmentOnce + 'db {
        let name = self.method.name(self.db);
        let static_prefix = self.method.is_static(self.db).then_some("static ");

        frag!(
            {static_prefix}{render_method_name(name)}({self.render_params()})~{{
                {self.render_body()}@
            }}
        )
    }
}

impl<'db, Casts> FunctionRenderer<'db, Casts> for MethodRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn db(self) -> &'db dyn Db {
        self.db
    }

    fn function(self) -> FacadeFunction<'db> {
        self.method.as_fn()
    }

    fn casts(self) -> Casts {
        self.casts
    }
}

#[derive(Copy, Clone)]
struct ConstructorRenderer<'db, Casts> {
    method: FacadeMethod<'db>,
    db: &'db dyn Db,
    casts: Casts,
}

impl<'db, Casts> FragmentOnce for ConstructorRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, Casts> ConstructorRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn render(self) -> impl FragmentOnce + 'db {
        let class_name = self.method.class(self.db).name(self.db);
        let body = self.render_param_casts(frag!(
            this.#_ffi~ = ~{self.render_call_expr()};
        ));

        frag!(
            constructor({self.render_params()})~{{
                if (arguments[0]~ === ~{class_name}.#_construction_token) {{
                    return;
                }}@
                {body}@
            }}
        )
    }
}

impl<'db, Casts> FunctionRenderer<'db, Casts> for ConstructorRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn db(self) -> &'db dyn Db {
        self.db
    }

    fn function(self) -> FacadeFunction<'db> {
        self.method.as_fn()
    }

    fn casts(self) -> Casts {
        self.casts
    }
}

trait FunctionRenderer<'db, Casts>: Copy + 'db
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn db(self) -> &'db dyn Db;

    fn function(self) -> FacadeFunction<'db>;

    fn casts(self) -> Casts;

    fn render_body(self) -> impl FragmentOnce + 'db {
        let db = self.db();
        let function = self.function();
        let ffi_ret_type = function.ffi(db).ret_type(db);
        let facade_ret_type = function.ret_type(db);

        fragment(move |p| match (ffi_ret_type, facade_ret_type) {
            (Some(ffi_ret_type), Some(facade_ret_type)) => {
                self.render_param_casts({
                    self.casts().render_stmt(
                        db,
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
        })
    }

    fn render_params(self) -> impl FragmentOnce {
        let db = self.db();
        comma_list(
            self.function()
                .params(db)
                .iter()
                .enumerate()
                .map(|(i, _)| param_name(i)),
        )
    }

    fn render_param_casts(self, continuation: impl FragmentOnce + 'db) -> impl FragmentOnce + 'db {
        let db = self.db();
        let function = self.function();
        let ffi_params = function.ffi(db).params(db);
        let receiver_offset = usize::from(matches!(
            function,
            FacadeFunction::Method(method) if !method.is_static(db)
        ));
        let ffi_params = &ffi_params[receiver_offset..];

        let param_casts = function.params(db).iter().zip(ffi_params).enumerate().map(
            |(i, (param, ffi_param))| Cast {
                from: param.type_(db),
                to: ffi_param.type_(db),
                in_expr: param_name(i),
                out_var_name: casted_param_name(i),
            },
        );

        self.casts().render_many(db, param_casts, continuation)
    }

    fn render_call_expr(self) -> impl FragmentOnce {
        let db = self.db();
        let function = self.function();
        let ffi_fn_symbol = function.ffi(db).symbol(db);
        let is_instance_method = matches!(
            function,
            FacadeFunction::Method(method) if !method.is_static(db)
        );
        let params = fragment(move |p| {
            if is_instance_method {
                frag!(this.#_ffi).apply(p);
                if !function.params(db).is_empty() {
                    frag!(,~).apply(p);
                }
            }

            comma_list(
                function
                    .params(db)
                    .iter()
                    .enumerate()
                    .map(|(i, _)| casted_param_name(i)),
            )
            .apply(p);
        });

        frag!({ ffi_fn_symbol }({ params }))
    }
}

fn param_name(i: usize) -> impl FragmentOnce {
    frag!(_{i})
}

fn casted_param_name(i: usize) -> impl FragmentOnce {
    frag!(__ { i })
}

fn render_method_name(name: &str) -> impl FragmentOnce + '_ {
    fragment(move |p| {
        if name == "constructor" {
            squares(doubles(name)).apply(p);
        } else {
            name.apply(p);
        }
    })
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
