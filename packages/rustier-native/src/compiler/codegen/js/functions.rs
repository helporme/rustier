use crate::compiler::{
    codegen::{
        fragment::{
            FragmentOnce, FragmentPrint, Printer, comma_list, doubles, frag, fragment, squares,
        },
        js::cast::{Cast, CastRenderer, Renderer},
    },
    db::Db,
    ir::facade::{FacadeFreeFunction, FacadeFunction, FacadeMethod, type_js::FacadeTypeJs},
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

    pub(super) fn render_as_constructor(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        let class_name = self.class(db).import_alias(db);
        let params = || {
            comma_list(
                self.params(db)
                    .enumerate()
                    .map(|(index, _)| param_name(index)),
            )
        };

        frag!(
            constructor({params()})~{{
                if (arguments[0]~ === ~{class_name}.#_construction_token) {{
                    return;
                }}@
                return ~{class_name}.new({params()});@
            }}
        )
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
        let decl_name = self.function.decl_name(self.db);

        fragment(move |p| {
            if name == decl_name {
                frag!(export~).apply(p);
            }

            frag!(
                function~{decl_name}~({self.render_params()})~{{
                    {self.render_body()}@
                }}
            )
            .apply(p);

            if name != decl_name {
                frag!(@@export~{{{decl_name}~as~{name}}};).apply(p);
            }
        })
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

        self.render_param_casts(fragment(move |p| {
            let target = function.ret_type_js(db);

            if matches!(target, FacadeTypeJs::Class(_)) {
                self.casts()
                    .render_stmt(
                        db,
                        Cast {
                            from: function.ret_type(db).abi(db),
                            to: target,
                            in_expr: self.render_call_expr(),
                            out_var_name: "result",
                        },
                        frag!(return result;),
                    )
                    .apply(p);
            } else {
                frag!(return ~{self.render_call_expr()};).apply(p);
            }
        }))
    }

    fn render_params(self) -> impl FragmentOnce {
        let db = self.db();
        comma_list(
            self.function()
                .params(db)
                .enumerate()
                .map(|(i, _)| param_name(i)),
        )
    }

    fn render_param_casts(self, continuation: impl FragmentOnce + 'db) -> impl FragmentOnce + 'db {
        let db = self.db();
        let function = self.function();
        let abi_params = function.abi(db).params(db);
        let receiver_offset = usize::from(matches!(
            function,
            FacadeFunction::Method(method) if !method.is_static(db)
        ));
        let abi_params = &abi_params[receiver_offset..];

        let param_casts =
            function
                .params(db)
                .zip(abi_params)
                .enumerate()
                .map(|(i, (param, abi_param))| Cast {
                    from: param.type_js(db),
                    to: *abi_param,
                    in_expr: param_name(i),
                    out_var_name: casted_param_name(i),
                });

        self.casts().render_many(db, param_casts, continuation)
    }

    fn render_call_expr(self) -> impl FragmentOnce {
        let db = self.db();
        let function = self.function();
        let abi_fn_symbol = function.abi(db).symbol(db);
        let is_instance_method = matches!(
            function,
            FacadeFunction::Method(method) if !method.is_static(db)
        );
        let params = fragment(move |p| {
            if is_instance_method {
                frag!(this.#_handle).apply(p);
                if function.params(db).next().is_some() {
                    frag!(,~).apply(p);
                }
            }

            comma_list(
                function
                    .params(db)
                    .enumerate()
                    .map(|(i, _)| casted_param_name(i)),
            )
            .apply(p);
        });

        frag!({ abi_fn_symbol }({ params }))
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
            ir::{facade::FacadeMember, rust::Rust},
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
                            .facade_complete(db)
                            .crates(db)
                            .next()
                            .expect("test crate should be lowered")
                            .root(db);
                        let mut functions = root.members(db).iter().copied().filter_map(|member| match member {
                            FacadeMember::Function(function) => Some(function),
                            _ => None,
                        });
                        let (Some(function), None) = (functions.next(), functions.next()) else {
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
