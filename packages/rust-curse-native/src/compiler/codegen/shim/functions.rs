use crate::compiler::{
    codegen::{
        fragment::{FragmentOnce, FragmentPrint as _, Printer, comma_list, frag, fragment},
        shim::{
            cast::{Cast, CastRenderer, Renderer},
            tethering::{
                TetheringStructType, render_tethering_struct_get_field_expr,
                render_tethering_struct_set_field_stmt,
            },
        },
    },
    db::Db,
    inputs::Platform,
    ir::{
        rust::{AsItem as _, Function, Shape, Type},
        shim::{ShimFunction, ShimFunctionDiscriminator, ShimFunctionGetter, ShimFunctionSetter},
    },
};

#[salsa::tracked]
impl<'db> ShimFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db) -> String {
        self.render_with(db, Renderer).print()
    }

    fn render_with<CastCompiler>(
        self,
        db: &'db dyn Db,
        cast_compiler: CastCompiler,
    ) -> impl FragmentOnce
    where
        CastCompiler: CastRenderer<'db> + Copy + Clone + 'db,
    {
        FunctionFragment {
            function: self,
            cast_compiler,
            db,
        }
    }
}

#[derive(Copy, Clone)]
struct FunctionFragment<'db, CastCompiler> {
    function: ShimFunction<'db>,
    cast_compiler: CastCompiler,
    db: &'db dyn Db,
}

impl<'db, CastCompiler> FragmentOnce for FunctionFragment<'db, CastCompiler>
where
    CastCompiler: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, CastCompiler> FunctionFragment<'db, CastCompiler>
where
    CastCompiler: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn render(self) -> impl FragmentOnce {
        let platform = Platform::get(self.db);

        let ret_type = self.function.ret_type(self.db);

        let result_type = fragment(move |p| match ret_type {
            Some(type_) => platform
                .result_type(type_.render_type_decl(self.db))
                .apply(p),

            None => platform.result_type("()").apply(p),
        });

        let symbol = self.function.symbol(self.db);

        frag!(
            {platform.header_renamed(symbol)}@
            pub fn ~{symbol}({self.render_params()})~ -> ~{result_type}~ {{
                {self.render_body()}
            }}
        )
    }

    fn render_params(self) -> impl FragmentOnce {
        comma_list(
            self.function
                .params(self.db)
                .iter()
                .enumerate()
                .map(move |(i, param)| {
                    fragment(move |p| {
                        frag!(
                            _{i}: ~{param.type_(self.db).render_type_decl(self.db)}
                        )
                        .apply(p);
                    })
                }),
        )
    }

    fn render_body(self) -> impl FragmentOnce {
        fragment(move |p| match self.function {
            ShimFunction::Free(free_fn) => self
                .render_source_fn_call_body(free_fn.src(self.db))
                .apply(p),
            ShimFunction::Associated(function) => self
                .render_source_fn_call_body(function.src(self.db))
                .apply(p),
            ShimFunction::Getter(getter) => self.render_fn_getter_body(getter).apply(p),
            ShimFunction::Setter(setter) => self.render_fn_setter_body(setter).apply(p),
            ShimFunction::EnumDiscriminator(discriminator) => {
                self.render_enum_discriminator_body(discriminator).apply(p)
            }
        })
    }

    fn render_source_fn_call_body(self, src_fn: Function<'db>) -> impl FragmentOnce {
        let cast_param_to_source_types =
            self.function
                .params(self.db)
                .iter()
                .enumerate()
                .map(|(i, param)| Cast {
                    from: param.type_(self.db),
                    to: param.src_type(self.db),
                    in_expr: frag!(_{i}),
                    out_var_name: frag!(_{i}),
                });

        let src_fn_call_expr = {
            let params = comma_list(
                self.function
                    .params(self.db)
                    .iter()
                    .enumerate()
                    .map(|(i, _)| frag!(_{i})),
            );
            frag!({ src_fn.fqn(self.db) }({ params }))
        };

        fragment(
            move |p| match (src_fn.ret_type(self.db), self.function.ret_type(self.db)) {
                (Some(src_ret_type), Some(shim_ret_type)) => {
                    let result_expr = self.cast_compiler.render_many(
                        self.db,
                        cast_param_to_source_types,
                        src_fn_call_expr,
                    );

                    let casted_result_expr = self.cast_compiler.render_expr(
                        self.db,
                        Type::from(src_ret_type),
                        shim_ret_type,
                        result_expr,
                    );

                    frag!(Ok({ casted_result_expr })).apply(p);
                }
                _ => {
                    let fn_call_stmt = self.cast_compiler.render_many(
                        self.db,
                        cast_param_to_source_types,
                        src_fn_call_expr,
                    );

                    frag!(
                        {fn_call_stmt};@
                        Ok(())
                    )
                    .apply(p);
                }
            },
        )
    }

    fn render_fn_getter_body(self, getter: ShimFunctionGetter<'db>) -> impl FragmentOnce {
        let [class_param] = getter.params(self.db);
        let cls_tethering_struct_expr = self.cast_compiler.render_expr(
            self.db,
            class_param.type_(self.db),
            TetheringStructType {
                source_type: class_param.src_type(self.db).deborrow(),
            },
            frag!(_0),
        );

        let field_tethering_value_expr = render_tethering_struct_get_field_expr(
            cls_tethering_struct_expr,
            getter.src(self.db).mangled_name(self.db),
        );

        let ret_type = getter.ret_type(self.db);

        let output = fragment(move |p| {
            if ret_type.uses_tethering() {
                self.cast_compiler
                    .render_expr(
                        self.db,
                        TetheringStructType {
                            source_type: getter.src(self.db).value_type(self.db),
                        },
                        ret_type,
                        field_tethering_value_expr,
                    )
                    .apply(p);
            } else {
                self.cast_compiler
                    .render_expr(
                        self.db,
                        Type::from(getter.src(self.db).value_type(self.db)),
                        ret_type,
                        field_tethering_value_expr,
                    )
                    .apply(p);
            }
        });

        frag!(Ok({ output }))
    }

    fn render_fn_setter_body(self, setter: ShimFunctionSetter<'db>) -> impl FragmentOnce {
        let [class_param, value_param] = setter.params(self.db);
        let cls_tethering_struct_expr = self.cast_compiler.render_expr(
            self.db,
            class_param.type_(self.db),
            TetheringStructType {
                source_type: class_param.src_type(self.db).deborrow(),
            },
            frag!(_0),
        );

        let set_tethered_field_stmt = render_tethering_struct_set_field_stmt(
            cls_tethering_struct_expr,
            setter.src(self.db).mangled_name(self.db),
            frag!(value),
        );

        let set_tethered_field_stmt = self.cast_compiler.render_stmt(
            self.db,
            Cast {
                from: value_param.type_(self.db),
                to: Type::Value(setter.src(self.db).value_type(self.db)),
                in_expr: frag!(_1),
                out_var_name: frag!(value),
            },
            set_tethered_field_stmt,
        );

        frag!(
            {set_tethered_field_stmt}
            Ok(())
        )
    }

    fn render_enum_discriminator_body(
        self,
        discriminator: ShimFunctionDiscriminator<'db>,
    ) -> impl FragmentOnce {
        let src = discriminator.src(self.db);
        let cls_tethering_struct_expr = self.cast_compiler.render_expr(
            self.db,
            discriminator.params(self.db)[0].type_(self.db),
            TetheringStructType {
                source_type: src.into(),
            },
            frag!(_0),
        );

        let variants = fragment(move |p| {
            let enum_name = src.fqn(self.db);

            for (index, variant) in src.variants(self.db).iter().enumerate() {
                let variant_name = variant.name(self.db);

                match variant.shape(self.db) {
                    Shape::Unit => {
                        frag!({enum_name}::{variant_name}~ => {index}f64,).apply(p);
                    }
                    Shape::Tuple(_) => {
                        frag!({enum_name}::{variant_name}(..) => {index}f64,).apply(p);
                    }
                    Shape::Plain { .. } => {
                        frag!({enum_name}::{variant_name}~{{ .. }} => {index}f64,).apply(p);
                    }
                }
            }

            if src.is_non_exhaustive(self.db) {
                let error = Platform::get(self.db).error(frag!(
                    "cannot discriminate an unknown non-exhaustive enum variant".to_string()
                ));
                frag!(_ => return Err({error}),).apply(p);
            }
        });

        frag!(
            let result = {cls_tethering_struct_expr}.access(|src|~{{
                Ok(match src~{{
                    {variants}
                }})
            }})?;@
            Ok(result)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_rust_snap,
        compiler::{codegen::shim::cast::test::TestCastRenderer, ir::rust::Rust},
    };

    fn find_test_function<'db>(
        db: &'db dyn Db,
        predicate: impl Fn(ShimFunction<'db>) -> bool,
    ) -> ShimFunction<'db> {
        let root = Rust::complete(db)
            .crates(db)
            .first()
            .expect("test crate should be lowered")
            .root(db)
            .shim(db);

        root.fns(db)
            .iter()
            .copied()
            .find(|function| predicate(*function))
            .expect("fixture should lower the requested shim function")
    }

    macro_rules! test_function {
        ($name:ident, $variant:pat, $($source:tt)*) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: { $($source)* },
                    expression: |db, printer| {
                        let db: &dyn Db = &*db;
                        let function = find_test_function(db, |function| {
                            matches!(function, $variant)
                        });

                        function
                            .render_with(db, TestCastRenderer)
                            .apply(printer);
                    },
                }
            }
        };
    }

    test_function!(
        zero_args_without_return,
        ShimFunction::Free(_),
        pub fn test_fn() {}
    );

    test_function!(
        zero_args_with_return,
        ShimFunction::Free(_),
        pub fn test_fn() -> i32 {
            0
        }
    );

    test_function!(
        one_arg_without_return,
        ShimFunction::Free(_),
        pub fn test_fn(_value: i32) {}
    );

    test_function!(
        one_arg_with_return,
        ShimFunction::Free(_),
        pub fn test_fn(value: i32) -> i32 {
            value
        }
    );

    test_function!(
        two_args_without_return,
        ShimFunction::Free(_),
        pub fn test_fn(_direct: i32, _borrowed: &i32) {}
    );

    test_function!(
        two_args_with_return,
        ShimFunction::Free(_),
        pub fn test_fn(direct: i32, borrowed: &i32) -> i32 {
            direct + borrowed
        }
    );

    test_function!(
        getter,
        ShimFunction::Getter(_),
        pub struct Test {
            pub field: i32,
        }
    );

    test_function!(
        setter,
        ShimFunction::Setter(_),
        pub struct Test {
            pub field: i32,
        }
    );

    test_function!(
        enum_discriminator,
        ShimFunction::EnumDiscriminator(_),
        pub enum Test {
            Unit,
            Tuple(i32),
            Plain { field: i32 },
        }
    );

    test_function!(
        no_self_associated_function,
        ShimFunction::Associated(_),
        pub struct Test;

        impl Test {
            pub fn create(value: i32) -> Self {
                let _ = value;
                Self
            }
        }
    );

    test_function!(
        owned_self_associated_function,
        ShimFunction::Associated(_),
        #[derive(Clone)]
        pub struct Test;

        impl Test {
            pub fn owned(self) -> bool {
                true
            }
        }
    );

    test_function!(
        shared_self_associated_function,
        ShimFunction::Associated(_),
        pub struct Test;

        impl Test {
            pub fn shared(&self, value: i32) -> i32 {
                value
            }
        }
    );

    test_function!(
        exclusive_self_associated_function,
        ShimFunction::Associated(_),
        pub struct Test;

        impl Test {
            pub fn exclusive(&mut self, value: i32) {
                let _ = value;
            }
        }
    );
}
