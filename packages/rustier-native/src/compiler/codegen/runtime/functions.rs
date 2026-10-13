use itertools::Either;

use crate::compiler::{
    codegen::{
        abi::{AbiArgs, AbiTransport},
        fragment::{FragmentOnce, FragmentPrint as _, Printer, comma_list, frag, fragment},
        runtime::cast::{Cast, CastRenderer, RuntimeCastRenderer},
    },
    db::Db,
    ir::{
        interface::InterClass,
        runtime::{
            RuntimeDiscriminatedCoerce, RuntimeDiscriminator, RuntimeFunction, RuntimeGetter,
            RuntimePlainCoerce, RuntimeSetter, tethering::RuntimeTetheredFieldKind,
        },
        rust::{Callable, Shape},
    },
};

#[salsa::tracked]
impl<'db> RuntimeFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db, transport: AbiTransport) -> String {
        self.render_with(db, transport, RuntimeCastRenderer).print()
    }

    fn render_with<Renderer>(
        self,
        db: &'db dyn Db,
        transport: AbiTransport,
        cast_compiler: Renderer,
    ) -> impl FragmentOnce
    where
        Renderer: CastRenderer<'db> + Copy + Clone + 'db,
    {
        FunctionFragment {
            function: self,
            transport,
            cast_compiler,
            db,
        }
    }
}

#[derive(Copy, Clone)]
struct FunctionFragment<'db, CastCompiler> {
    function: RuntimeFunction<'db>,
    transport: AbiTransport,
    cast_compiler: CastCompiler,
    db: &'db dyn Db,
}

#[derive(Copy, Clone)]
struct CoerceFnFragment<'db, CastCompiler> {
    coerce: Either<RuntimePlainCoerce<'db>, RuntimeDiscriminatedCoerce<'db>>,
    args: AbiArgs<'db>,
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
                    RuntimeFunction::from(coerce),
                    src.path(db).fqn(db),
                    src.def(db).shape(db),
                    src.into_type(db),
                )
            }
            Either::Right(coerce) => {
                let src = coerce.variant(db).src(db);
                (
                    RuntimeFunction::from(coerce),
                    src.path(db).fqn(db),
                    src.declaration(db).shape(db),
                    src.target(db).into_type(db),
                )
            }
        };
        let params = function.params(db);
        let casts = params.iter().enumerate().map(move |(i, param)| Cast {
            from: self.args.get(i).type_(),
            to: *param,
            in_expr: self.args.get(i).expr(),
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
                .abi(db)
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
        self.function
            .abi(self.db)
            .render_rust_with_body(self.db, self.transport, move |args| self.render_body(args))
    }

    fn render_body(self, args: AbiArgs<'db>) -> impl FragmentOnce {
        fragment(move |p| match self.function {
            RuntimeFunction::Free(free_fn) => self
                .render_source_fn_call_body(free_fn.src(self.db), args)
                .apply(p),
            RuntimeFunction::Associated(function) => self
                .render_source_fn_call_body(function.src(self.db), args)
                .apply(p),
            RuntimeFunction::Getter(getter) => self.render_fn_getter_body(getter, args).apply(p),
            RuntimeFunction::Setter(setter) => self.render_fn_setter_body(setter, args).apply(p),
            RuntimeFunction::Discriminator(discriminator) => self
                .render_enum_discriminator_body(discriminator, args)
                .apply(p),
            RuntimeFunction::PlainCoerce(coerce) => CoerceFnFragment {
                coerce: Either::Left(coerce),
                args,
                cast_compiler: self.cast_compiler,
                db: self.db,
            }
            .apply(p),
            RuntimeFunction::DiscriminatedCoerce(coerce) => CoerceFnFragment {
                coerce: Either::Right(coerce),
                args,
                cast_compiler: self.cast_compiler,
                db: self.db,
            }
            .apply(p),
        })
    }

    fn render_source_fn_call_body(
        self,
        src_fn: Callable<'db>,
        args: AbiArgs<'db>,
    ) -> impl FragmentOnce {
        let cast_param_to_source_types = self
            .function
            .params(self.db)
            .iter()
            .copied()
            .enumerate()
            .map(move |(i, param)| Cast {
                from: args.get(i).type_(),
                to: param,
                in_expr: args.get(i).expr(),
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

        fragment(move |p| {
            match (
                src_fn.ret_type(self.db),
                self.function.abi(self.db).ret_type(self.db),
            ) {
                (Some(src_ret_type), Some(abi_ret_type)) => {
                    let result_expr = self.cast_compiler.render_many(
                        self.db,
                        cast_param_to_source_types,
                        src_fn_call_expr,
                    );

                    let casted_result_expr = self.cast_compiler.render_expr(
                        self.db,
                        src_ret_type,
                        abi_ret_type,
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
            }
        })
    }

    fn render_fn_getter_body(
        self,
        getter: RuntimeGetter<'db>,
        args: AbiArgs<'db>,
    ) -> impl FragmentOnce {
        let tethered_field = getter.field(self.db).tethered(self.db);
        let field = tethered_field.src(self.db);

        let cls_tethering_struct_expr = self.cast_compiler.render_expr(
            self.db,
            args.get(0).type_(),
            tethered_field.parent(self.db),
            args.get(0).expr(),
        );

        let field_tethering_value_expr =
            tethered_field.render_get_expr(self.db, cls_tethering_struct_expr);

        let ret_type = self
            .function
            .abi(self.db)
            .ret_type(self.db)
            .expect("getters return the field value");

        let output = fragment(move |p| match tethered_field.kind(self.db) {
            RuntimeTetheredFieldKind::Tethered(tethering) => self
                .cast_compiler
                .render_expr(self.db, tethering, ret_type, field_tethering_value_expr)
                .apply(p),
            RuntimeTetheredFieldKind::Cloned => self
                .cast_compiler
                .render_expr(
                    self.db,
                    field._type(self.db),
                    ret_type,
                    field_tethering_value_expr,
                )
                .apply(p),
        });

        frag!(Ok({ output }))
    }

    fn render_fn_setter_body(
        self,
        setter: RuntimeSetter<'db>,
        args: AbiArgs<'db>,
    ) -> impl FragmentOnce {
        let tethered_field = setter.field(self.db).tethered(self.db);
        let field = tethered_field.src(self.db);

        let cls_tethering_struct_expr = self.cast_compiler.render_expr(
            self.db,
            args.get(0).type_(),
            tethered_field.parent(self.db),
            args.get(0).expr(),
        );

        let set_tethered_field_stmt =
            tethered_field.render_set_stmt(self.db, cls_tethering_struct_expr, frag!(value));

        let set_tethered_field_stmt = self.cast_compiler.render_stmt(
            self.db,
            Cast {
                from: args.get(1).type_(),
                to: field._type(self.db),
                in_expr: args.get(1).expr(),
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
        discriminator: RuntimeDiscriminator<'db>,
        args: AbiArgs<'db>,
    ) -> impl FragmentOnce {
        let src = discriminator.class(self.db).src(self.db);
        let cls_tethering_struct_expr = self.cast_compiler.render_expr(
            self.db,
            args.get(0).type_(),
            InterClass::Discriminated(discriminator.class(self.db)).tethering(self.db),
            args.get(0).expr(),
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
                frag!(_ => return Err(rustier_rt::EnumError::UnknownVariant.into()),).apply(p);
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
        compiler::{codegen::runtime::cast::test::TestCastRenderer, ir::rust::Rust},
    };

    fn find_test_function<'db>(
        db: &'db dyn Db,
        predicate: impl Fn(RuntimeFunction<'db>) -> bool,
    ) -> RuntimeFunction<'db> {
        Rust::get(db)
            .interface_complete(db)
            .runtime(db)
            .functions(db)
            .iter()
            .copied()
            .find(|function| predicate(*function))
            .expect("fixture should lower the requested runtime function")
    }

    macro_rules! test_function {
        ($name:ident, $variant:pat, $($source:tt)*) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: { $($source)* },
                    expression: |db, transport, printer| {
                        let db: &dyn Db = &*db;
                        let function = find_test_function(db, |function| {
                            matches!(function, $variant)
                        });

                        function
                            .render_with(db, transport, TestCastRenderer)
                            .apply(printer);
                    },
                }
            }
        };
    }

    test_function!(
        zero_args_without_return,
        RuntimeFunction::Free(_),
        pub fn test_fn() {}
    );

    macro_rules! test_coerce {
        ($name:ident, $target:literal, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: { $($source)* },
                    expression: |db, transport, printer| {
                        let db: &dyn Db = &*db;
                        let function = find_test_function(db, |function| match function {
                            RuntimeFunction::PlainCoerce(coerce) => {
                                coerce.class(db).name(db) == $target
                            }
                            RuntimeFunction::DiscriminatedCoerce(coerce) => {
                                coerce.variant(db).name(db) == $target
                            }
                            _ => false,
                        });
                        function.render(db, transport).apply(printer);
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
        RuntimeFunction::Free(_),
        pub fn test_fn() -> i32 {
            0
        }
    );

    test_function!(
        one_arg_without_return,
        RuntimeFunction::Free(_),
        pub fn test_fn(_value: i32) {}
    );

    test_function!(
        one_arg_with_return,
        RuntimeFunction::Free(_),
        pub fn test_fn(value: i32) -> i32 {
            value
        }
    );

    test_function!(
        two_args_without_return,
        RuntimeFunction::Free(_),
        pub fn test_fn(_direct: i32, _borrowed: &i32) {}
    );

    test_function!(
        two_args_with_return,
        RuntimeFunction::Free(_),
        pub fn test_fn(direct: i32, borrowed: &i32) -> i32 {
            direct + borrowed
        }
    );

    test_function!(
        getter,
        RuntimeFunction::Getter(_),
        pub struct Test {
            pub field: i32,
        }
    );

    test_function!(
        setter,
        RuntimeFunction::Setter(_),
        pub struct Test {
            pub field: i32,
        }
    );

    test_function!(
        enum_discriminator,
        RuntimeFunction::Discriminator(_),
        pub enum Test {
            Unit,
            Tuple(i32),
            Plain { field: i32 },
        }
    );

    test_function!(
        no_self_associated_function,
        RuntimeFunction::Associated(_),
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
        RuntimeFunction::Associated(_),
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
        RuntimeFunction::Associated(_),
        pub struct Test;

        impl Test {
            pub fn shared(&self, value: i32) -> i32 {
                value
            }
        }
    );

    test_function!(
        exclusive_self_associated_function,
        RuntimeFunction::Associated(_),
        pub struct Test;

        impl Test {
            pub fn exclusive(&mut self, value: i32) {
                let _ = value;
            }
        }
    );
}
