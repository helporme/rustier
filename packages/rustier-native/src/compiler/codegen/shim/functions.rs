use itertools::Either;

use crate::compiler::{
    codegen::{
        fragment::{FragmentOnce, FragmentPrint as _, Printer, comma_list, frag, fragment},
        shim::{
            Platform,
            cast::{Cast, CastRenderer, ShimCastRenderer},
            tethering::{
                TetheringStructType, render_tethering_struct_get_field_expr,
                render_tethering_struct_set_field_stmt,
            },
        },
    },
    db::Db,
    ir::{
        rust::{Callable, Shape},
        shim::{
            ShimFunction, ShimFunctionDiscriminatedCoerce, ShimFunctionDiscriminator,
            ShimFunctionGetter, ShimFunctionPlainCoerce, ShimFunctionSetter,
        },
    },
};

#[salsa::tracked]
impl<'db> ShimFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db, platform: Platform) -> String {
        self.render_with(db, platform, ShimCastRenderer { platform })
            .print()
    }

    fn render_with<Renderer>(
        self,
        db: &'db dyn Db,
        platform: Platform,
        cast_compiler: Renderer,
    ) -> impl FragmentOnce
    where
        Renderer: CastRenderer<'db> + Copy + Clone + 'db,
    {
        FunctionFragment {
            function: self,
            platform,
            cast_compiler,
            db,
        }
    }
}

#[derive(Copy, Clone)]
struct FunctionFragment<'db, CastCompiler> {
    function: ShimFunction<'db>,
    platform: Platform,
    cast_compiler: CastCompiler,
    db: &'db dyn Db,
}

#[derive(Copy, Clone)]
struct CoerceFnFragment<'db, CastCompiler> {
    coerce: Either<ShimFunctionPlainCoerce<'db>, ShimFunctionDiscriminatedCoerce<'db>>,
    cast_compiler: CastCompiler,
    db: &'db dyn Db,
}

impl<'db, CastCompiler> FragmentOnce for CoerceFnFragment<'db, CastCompiler>
where
    CastCompiler: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        let db = self.db;
        let (function, path, shape, source_type) = match self.coerce {
            Either::Left(coerce) => {
                let src = coerce.class(db).src(db);
                (
                    ShimFunction::from(coerce),
                    src.path(db).fqn(db),
                    src.def(db).shape(db),
                    src.into_type(db),
                )
            }
            Either::Right(coerce) => {
                let src = coerce.variant(db).src(db);
                (
                    ShimFunction::from(coerce),
                    src.path(db).fqn(db),
                    src.declaration(db).shape(db),
                    src.target(db).into_type(db),
                )
            }
        };
        let params = function.params(db);
        let casts = params.iter().enumerate().map(|(i, param)| Cast {
            from: param.type_(db),
            to: param.src_type(db),
            in_expr: frag!(_{i}),
            out_var_name: frag!(_{i}),
        });
        let value = fragment(move |p| match shape {
            Shape::Unit => path.apply(p),
            Shape::Tuple(_) => {
                let args = comma_list(params.iter().enumerate().map(|(i, _)| frag!(_{i})));
                frag!({ path }({ args })).apply(p);
            }
            Shape::Plain { fields, .. } => {
                let fields = comma_list(fields.iter().enumerate().map(|(i, field)| {
                    let name = field.escaped_name(db);
                    frag!({name}: _{i})
                }));
                frag!({path}~{{ {fields} }}).apply(p);
            }
        });
        let value = self.cast_compiler.render_many(db, casts, value);
        let result = self.cast_compiler.render_expr(
            db,
            source_type,
            function
                .ret_type(db)
                .expect("coerce methods return a class"),
            value,
        );
        frag!(Ok({ result })).apply(p);
    }
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
        let platform = self.platform;

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
                .copied()
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
            ShimFunction::PlainCoerce(coerce) => CoerceFnFragment {
                coerce: Either::Left(coerce),
                cast_compiler: self.cast_compiler,
                db: self.db,
            }
            .apply(p),
            ShimFunction::DiscriminatedCoerce(coerce) => CoerceFnFragment {
                coerce: Either::Right(coerce),
                cast_compiler: self.cast_compiler,
                db: self.db,
            }
            .apply(p),
        })
    }

    fn render_source_fn_call_body(self, src_fn: Callable<'db>) -> impl FragmentOnce {
        let cast_param_to_source_types = self
            .function
            .params(self.db)
            .iter()
            .copied()
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
            frag!({ src_fn.path(self.db).fqn(self.db) }({ params }))
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
                        src_ret_type,
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
                source_type: class_param.src_type(self.db).deborrow(self.db),
            },
            frag!(_0),
        );

        let field_tethering_value_expr = render_tethering_struct_get_field_expr(
            cls_tethering_struct_expr,
            getter
                .src(self.db)
                .declaration(self.db)
                .mangled_name(self.db),
        );

        let ret_type = getter.ret_type(self.db);

        let output = fragment(move |p| {
            if ret_type.uses_tethering() {
                self.cast_compiler
                    .render_expr(
                        self.db,
                        TetheringStructType {
                            source_type: getter.src(self.db)._type(self.db),
                        },
                        ret_type,
                        field_tethering_value_expr,
                    )
                    .apply(p);
            } else {
                self.cast_compiler
                    .render_expr(
                        self.db,
                        getter.src(self.db)._type(self.db),
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
                source_type: class_param.src_type(self.db).deborrow(self.db),
            },
            frag!(_0),
        );

        let set_tethered_field_stmt = render_tethering_struct_set_field_stmt(
            cls_tethering_struct_expr,
            setter
                .src(self.db)
                .declaration(self.db)
                .mangled_name(self.db),
            frag!(value),
        );

        let set_tethered_field_stmt = self.cast_compiler.render_stmt(
            self.db,
            Cast {
                from: value_param.type_(self.db),
                to: setter.src(self.db)._type(self.db),
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
                source_type: src.into_type(self.db),
            },
            frag!(_0),
        );

        let variants = fragment(move |p| {
            let enum_name = src.path(self.db).fqn(self.db);

            for (index, variant) in src.variants(self.db).enumerate() {
                let variant_name = variant.declaration(self.db).name(self.db);

                // This pattern works for every shape, including
                // non-exhaustive unit and tuple variants.
                frag!({enum_name}::{variant_name}~{{ .. }} => {index}f64,).apply(p);
            }

            if src.def(self.db).is_non_exhaustive(self.db) {
                let error = self.platform.error(frag!(
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
        let root = Rust::get(db)
            .shim_complete(db)
            .crates(db)
            .next()
            .expect("test crate should be lowered")
            .root(db);

        root.functions(db)
            .map(ShimFunction::from)
            .chain(
                root.classes(db)
                    .flat_map(|class| class.all_functions(db).iter().copied()),
            )
            .find(|function| predicate(*function))
            .expect("fixture should lower the requested shim function")
    }

    macro_rules! test_function {
        ($name:ident, $variant:pat, $($source:tt)*) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: { $($source)* },
                    expression: |db, platform, printer| {
                        let db: &dyn Db = &*db;
                        let function = find_test_function(db, |function| {
                            matches!(function, $variant)
                        });

                        function
                            .render_with(db, platform, TestCastRenderer)
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

    macro_rules! test_coerce {
        ($name:ident, $target:literal, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: { $($source)* },
                    expression: |db, platform, printer| {
                        let db: &dyn Db = &*db;
                        let function = find_test_function(db, |function| match function {
                            ShimFunction::PlainCoerce(coerce) => {
                                coerce.class(db).name(db) == $target
                            }
                            ShimFunction::DiscriminatedCoerce(coerce) => {
                                coerce.variant(db).name(db) == $target
                            }
                            _ => false,
                        });
                        function.render(db, platform).apply(printer);
                    },
                }
            }
        };
    }

    test_coerce!(struct_coerce_unit, "Unit", {
        #[derive(Clone)]
        pub struct Unit;
    });

    test_coerce!(struct_coerce_empty_tuple, "EmptyTuple", {
        #[derive(Clone)]
        pub struct EmptyTuple();
    });

    test_coerce!(struct_coerce_empty_record, "EmptyRecord", {
        #[derive(Clone)]
        pub struct EmptyRecord {}
    });

    test_coerce!(struct_coerce_tuple, "Tuple", {
        #[derive(Clone)]
        pub struct Tuple(pub i32, pub String);
    });

    test_coerce!(struct_coerce_record, "Record", {
        #[derive(Clone)]
        pub struct Record {
            pub number: f64,
            pub flag: bool,
        }
    });

    test_coerce!(struct_coerce_raw_field, "RawField", {
        #[derive(Clone)]
        pub struct RawField {
            pub r#type: u32,
        }
    });

    test_coerce!(struct_coerce_nested, "Nested", {
        #[derive(Clone)]
        pub struct Child {
            pub value: i32,
        }

        #[derive(Clone)]
        pub struct Nested {
            pub child: Child,
        }
    });

    test_coerce!(struct_coerce_shadowed_method, "Record", {
        #[derive(Clone)]
        pub struct Record {
            pub number: f64,
        }

        impl Record {
            pub fn number(&self) -> f64 {
                self.number
            }
        }
    });

    test_coerce!(variant_coerce_unit, "Unit", {
        #[derive(Clone)]
        pub enum Test {
            Unit,
        }
    });

    test_coerce!(variant_coerce_empty_tuple, "EmptyTuple", {
        #[derive(Clone)]
        pub enum Test {
            EmptyTuple(),
        }
    });

    test_coerce!(variant_coerce_empty_record, "EmptyRecord", {
        #[derive(Clone)]
        pub enum Test {
            EmptyRecord {},
        }
    });

    test_coerce!(variant_coerce_tuple, "Tuple", {
        #[derive(Clone)]
        pub enum Test {
            Tuple(i32, String),
        }
    });

    test_coerce!(variant_coerce_record, "Record", {
        #[derive(Clone)]
        pub enum Test {
            Record { text: String, flag: bool },
        }
    });

    test_coerce!(variant_coerce_raw_field, "RawField", {
        #[derive(Clone)]
        pub enum Test {
            RawField { r#type: u32 },
        }
    });

    test_coerce!(variant_coerce_nested, "Nested", {
        #[derive(Clone)]
        pub struct Child(pub i32);

        #[derive(Clone)]
        pub enum Test {
            Nested(Child),
        }
    });

    test_coerce!(variant_coerce_non_exhaustive_parent, "Unit", {
        #[derive(Clone)]
        #[non_exhaustive]
        pub enum Test {
            Unit,
            #[non_exhaustive]
            Open,
        }
    });

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
