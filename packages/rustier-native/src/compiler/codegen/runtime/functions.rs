use itertools::Either;

use crate::compiler::{
    codegen::{
        abi::{AbiArgs, AbiFunctionRenderOps},
        fragment::{FragmentOnce, FragmentPrint as _, Printer, comma_list, frag, fragment},
        runtime::cast::{Cast, CastRenderer, RuntimeCastRenderer},
    },
    db::Db,
    ir::{
        abi::{AbiReturn, transport::AbiTransport},
        facade::FacadeClass,
        runtime::{
            RuntimeDiscriminatedCoerce, RuntimeDiscriminator, RuntimeFunction, RuntimeGetter,
            RuntimePlainCoerce, RuntimeSetter, RuntimeType, tethering::RuntimeTetheredFieldKind,
        },
        rust::{Callable, Shape, Type, TypeKind},
    },
};

#[salsa::tracked]
impl<'db> RuntimeFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_rust(self, db: &'db dyn Db, transport: AbiTransport) -> String {
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
        let ret_type = source_type.abi(db);

        let result = self
            .cast_compiler
            .render_expr(db, source_type, ret_type, value);
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
        let abi = self.function.abi(self.db);
        let abi_return = abi.ret_type(self.db);
        let ret_type = self.function.ret_type(self.db);
        let source = match self.function {
            RuntimeFunction::Free(function) => Some(function.src(self.db)),
            RuntimeFunction::Associated(function) => Some(function.src(self.db)),
            _ => None,
        };
        let ops = AbiFunctionRenderOps {
            transport: self.transport,
            inject_js_env: source.is_some_and(|source| {
                source
                    .params(self.db)
                    .iter()
                    .any(|param| param.type_(self.db).is_js_env(self.db))
            }),
        };

        abi.render_rust_with_body(self.db, ops, move |args| {
            fragment(move |p| {
                let AbiReturn::Union(union) = abi_return else {
                    self.render_body(args).apply(p);
                    return;
                };

                let success = union.render_rust_variant(0, "value");
                let user_error = union.render_rust_variant(1, "error");
                let failure = union.render_rust_variant(
                    union.variants(self.db).len() - 1,
                    frag!(error.to_string()),
                );
                let branches = fragment(move |p| match ret_type {
                    RuntimeType::Result { .. } => frag!(
                        Ok(core::result::Result::Ok(value)) => {success},@
                        Ok(core::result::Result::Err(error)) => {user_error},@
                    )
                    .apply(p),
                    _ => frag!(
                        Ok(value) => {success},@
                    )
                    .apply(p),
                });

                frag!(
                    let result = (|| -> rustier_rt::CallResult<{ret_type.render_rust(self.db)}> {{
                        {self.render_body(args)}
                    }})();@@

                    match result {{
                        {branches}
                        Err(error) => {failure},
                    }}
                )
                .apply(p);
            })
        })
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
            RuntimeFunction::DropHandle(drop) => {
                let target = drop.tethering(self.db).tethering_struct_name(self.db);
                let handle = args.get(0).expr();

                frag!(unsafe {
                    {
                        rustier_rt::rust_handles::drop_handle::<{ target }>({ handle });
                    }
                })
                .apply(p);
            }
        })
    }

    fn render_source_fn_call(self, src_fn: Callable<'db>, args: AbiArgs<'db>) -> impl FragmentOnce {
        let params = self.function.params(self.db);

        // Owned arguments must be cloned before reference borrows are held, so an
        // owned argument can alias a mutable argument or receiver regardless of
        // parameter order.
        let owned_params = params
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, param)| !matches!(param.kind(self.db), TypeKind::Borrowed(_)));

        let borrowed_params = params
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, param)| matches!(param.kind(self.db), TypeKind::Borrowed(_)));

        let cast_param_to_source_types =
            owned_params
                .chain(borrowed_params)
                .map(move |(i, param)| Cast {
                    from: args.get(i).type_(),
                    to: param,
                    in_expr: args.get(i).expr(),
                    out_var_name: frag!(_{i}),
                });

        let src_fn_call_expr = {
            let mut abi_index = 0;
            let params = comma_list(
                src_fn
                    .receiver_type(self.db)
                    .into_iter()
                    .chain(
                        src_fn
                            .params(self.db)
                            .iter()
                            .map(|param| *param.type_(self.db)),
                    )
                    .map(move |type_| {
                        let index = if type_.is_js_env(self.db) {
                            None
                        } else {
                            let index = abi_index;
                            abi_index += 1;

                            Some(index)
                        };

                        fragment(move |p| match index {
                            Some(index) => frag!(_{index}).apply(p),
                            None => frag!(__js_env).apply(p),
                        })
                    }),
            );

            frag!({ src_fn.path(self.db).fqn(self.db) }({ params }))
        };

        self.cast_compiler
            .render_many(self.db, cast_param_to_source_types, src_fn_call_expr)
    }

    fn render_source_fn_call_body(
        self,
        src_fn: Callable<'db>,
        args: AbiArgs<'db>,
    ) -> impl FragmentOnce {
        let src_ret_type = src_fn.ret_type(self.db);

        fragment(move |p| match self.function.ret_type(self.db) {
            RuntimeType::Result { ok, err } => {
                self.render_source_fn_result_body(src_fn, args, ok, err)
                    .apply(p);
            }
            _ => {
                let casted_result_expr = self.cast_compiler.render_expr(
                    self.db,
                    src_ret_type,
                    src_ret_type.abi(self.db),
                    self.render_source_fn_call(src_fn, args),
                );

                frag!(Ok({ casted_result_expr })).apply(p);
            }
        })
    }

    fn render_source_fn_result_body(
        self,
        src_fn: Callable<'db>,
        args: AbiArgs<'db>,
        ok: Type<'db>,
        err: Type<'db>,
    ) -> impl FragmentOnce {
        let ok_value = self
            .cast_compiler
            .render_expr(self.db, ok, ok.abi(self.db), "value");
        let err_value = self
            .cast_compiler
            .render_expr(self.db, err, err.abi(self.db), "error");
        frag!(
            Ok(match ~{self.render_source_fn_call(src_fn, args)}~{{
                core::result::Result::Ok(value) => core::result::Result::Ok({ok_value}),@
                core::result::Result::Err(error) => core::result::Result::Err({err_value}),
            }})
        )
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

        let ret_type = field._type(self.db).abi(self.db);

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
            FacadeClass::Discriminated(discriminator.class(self.db)).tethering(self.db),
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
        compiler::{
            codegen::{
                fragment::FragmentPrint,
                runtime::{cast::test::TestCastRenderer, test::build_rust_snapshot},
            },
            ir::{
                abi::transport::{WasmMemory, WasmTransport},
                rust::Rust,
            },
            test::build_js_env_test_db,
        },
    };

    fn find_test_function<'db>(
        db: &'db dyn Db,
        predicate: impl Fn(RuntimeFunction<'db>) -> bool,
    ) -> RuntimeFunction<'db> {
        Rust::get(db)
            .facade_complete(db)
            .runtime(db)
            .functions(db)
            .iter()
            .copied()
            .find(|function| predicate(*function))
            .expect("fixture should lower the requested runtime function")
    }

    fn snapshot_js_env(name: &str, source: &str) {
        let db = build_js_env_test_db(source);
        let (node, wasm) = ra_ap_hir::attach_db(&db, || {
            let function = find_test_function(&db, |function| {
                matches!(
                    function,
                    RuntimeFunction::Free(_) | RuntimeFunction::Associated(_)
                )
            });
            let node = function
                .render_with(&db, AbiTransport::Napi, TestCastRenderer)
                .print();
            let wasm = function
                .render_with(
                    &db,
                    AbiTransport::Wasm(WasmTransport {
                        memory: WasmMemory::x32,
                    }),
                    TestCastRenderer,
                )
                .print();

            (node, wasm)
        });
        let (snapshot, snapshot_path) = build_rust_snapshot(source, &node, &wasm, file!());

        insta::with_settings!({
            snapshot_path => snapshot_path,
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!(name, snapshot);
        });
    }

    #[test]
    fn js_env_only() {
        snapshot_js_env(
            "js_env_only",
            r#"
                use rustier_rt::js::JsEnv;

                pub fn record_env(_env: JsEnv) {}
            "#,
        );
    }

    #[test]
    fn js_env_mixed_args() {
        snapshot_js_env(
            "js_env_mixed_args",
            r#"
                pub use rustier_rt::js::JsEnv as Environment;
                type Alias = Environment;

                pub fn combine(
                    _first: Environment,
                    left: i32,
                    _middle: Alias,
                    right: &i32,
                    _last: Environment,
                ) -> i32 {
                    left + right
                }
            "#,
        );
    }

    #[test]
    fn js_env_method() {
        snapshot_js_env(
            "js_env_method",
            r#"
                use rustier_rt::js::JsEnv;

                pub struct Counter;

                impl Counter {
                    pub fn combine(&self, left: i32, _env: JsEnv, right: i32) -> i32 {
                        left + right
                    }
                }
            "#,
        );
    }

    macro_rules! test_function {
        ($name:ident, $variant:pat, $($source:tt)*) => {
            test_function!(@fixture $name, $variant, [], $($source)*);
        };
        (@fixture $name:ident, $variant:pat, [$($settings:tt)*], $($source:tt)*) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    $($settings)*
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
                        function.render_rust(db, transport).apply(printer);
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
        @fixture result_return,
        RuntimeFunction::Free(_),
        [minicore: "clone, derive, result",],
        pub fn test_fn() -> Result<i32, String> {
            loop {}
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
