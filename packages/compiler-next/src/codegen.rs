use crate::{
    db::Db,
    fragment::{FragmentOnce, FragmentPrint, frag, fragment},
    transport::{Backend, Function, Module, Part, Type},
};

#[salsa::tracked]
impl<'db> Function<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db, backend: Backend) -> String {
        let name = self.name(db);
        let return_type = self.return_type(db);
        let return_source = return_type.source(db);
        let arguments = self.body(db).iter().filter_map(|part| match part {
            Part::Text(_) => None,
            Part::Interpolation { type_ } => Some(*type_),
        });

        let params = fragment(|printer| {
            for (index, _) in arguments.clone().enumerate() {
                frag!(, ~__rustier_raw_{index}: u32).apply(printer);
            }
        });

        let owned_inputs = fragment(|printer| {
            if backend == Backend::Node {
                for (index, type_) in arguments.clone().enumerate() {
                    if matches!(type_, Type::Owned(_)) {
                        frag!(
                            let __rustier_raw_{index} =
                                ::runtime_next::runtime::js::JsValue::from(unsafe {{
                                    ::runtime_next::transport::JsValueHandle::from_napi_value(
                                        __rustier_values[{index}],
                                        ::runtime_next::transport::JsEnvHandle::from_raw(__rustier_env_raw)
                                    )
                                }}).as_u32().expect("expected a Rust handle ID");@
                        )
                        .apply(printer);
                    }
                }
            }
        });

        let validation = fragment(|printer| {
            let handles = fragment(|printer| {
                for (index, type_) in arguments.clone().enumerate() {
                    if matches!(type_, Type::Owned(_)) {
                        frag!(__rustier_raw_ { index },).apply(printer);
                    }
                }
            });

            frag!(
                ::runtime_next::runtime::rust_handles::validate_distinct(
                    &[{handles}]
                ).expect("cannot move the same Rust handle twice");@
            )
            .apply(printer);

            for (index, type_) in arguments.clone().enumerate() {
                if let Type::Owned(type_) = type_ {
                    let source = type_.source(db);

                    frag!(
                        ::runtime_next::runtime::rust_handles::RustHandle::from_raw(
                            __rustier_raw_{index}
                        ).validate::<{source}>().expect("invalid Rust handle or type");@
                    )
                    .apply(printer);
                }
            }
        });

        let conversions = fragment(|printer| {
            for (index, type_) in arguments.clone().enumerate() {
                if matches!(type_, Type::Owned(_)) {
                    continue;
                }

                let handle = fragment(|printer| match backend {
                    Backend::Wasm => {
                        frag!(::runtime_next::transport::JsValueHandle::clone_from_raw(
                            __rustier_raw_ { index }
                        ))
                        .apply(printer)
                    }
                    Backend::Node => {
                        frag!(::runtime_next::transport::JsValueHandle::from_napi_value(
                            __rustier_values[{ index }],
                            ::runtime_next::transport::JsEnvHandle::from_raw(__rustier_env_raw)
                        ))
                        .apply(printer)
                    }
                });

                frag!(
                    let __rustier_js_{index} =
                        ::runtime_next::runtime::js::JsValue::from(unsafe {{
                            {handle}
                        }});@
                )
                .apply(printer);

                let source = type_.source(db);
                let value = fragment(|printer| {
                    frag!(__rustier_js_ { index }).apply(printer);

                    if let Some(method) = type_.js_conversion() {
                        frag!(.{method}().expect("JavaScript value cannot be converted"))
                            .apply(printer);
                    }
                });

                frag!(let __rustier_arg_{index}: ~{source} = {value};@).apply(printer);
            }

            for (index, type_) in arguments.clone().enumerate() {
                if let Type::Owned(type_) = type_ {
                    let source = type_.source(db);

                    frag!(
                        let __rustier_arg_{index}: ~{source} =
                            ::runtime_next::runtime::rust_handles::RustHandle::from_raw(
                                __rustier_raw_{index}
                            ).take::<{source}>().expect("invalid Rust handle or type");@
                    )
                    .apply(printer);
                }
            }
        });

        let body = fragment(|printer| {
            let mut index = 0;
            for part in self.body(db) {
                match part {
                    Part::Text(text) => text.apply(printer),
                    Part::Interpolation { .. } => {
                        frag!(__rustier_arg_ { index }).apply(printer);
                        index += 1;
                    }
                }
            }
        });

        let export = fragment(|printer| {
            if backend == Backend::Wasm && matches!(return_type, Type::Owned(_)) {
                frag!(
                    ::runtime_next::runtime::rust_handles::RustHandle::new(__rustier_output)
                        .into_raw()
                )
                .apply(printer);
                return;
            }

            let value = fragment(|printer| match return_type {
                Type::Owned(_) => frag!(
                    __rustier_env.new_number(
                        ::runtime_next::runtime::rust_handles::RustHandle::new(__rustier_output)
                            .into_raw() as f64
                    )
                )
                .apply(printer),
                Type::JsValue => frag!(__rustier_output).apply(printer),
                Type::String => frag!(__rustier_env.new_string(__rustier_output)).apply(printer),
                _ => frag!(__rustier_env.new_number(__rustier_output as f64)).apply(printer),
            });

            match backend {
                Backend::Wasm => {
                    frag!(::runtime_next::transport::JsValueHandle::from({ value }).into_raw())
                        .apply(printer);
                }
                Backend::Node => {
                    frag!(unsafe ~{{
                        ::runtime_next::transport::node::export(
                            ::runtime_next::transport::JsValueHandle::from({value}),
                            __rustier_env_raw
                        )
                    }})
                    .apply(printer);
                }
            }
        });

        let signature = fragment(|printer| match backend {
            Backend::Wasm => frag!(
                #[unsafe(no_mangle)]@
                pub unsafe extern "C" fn ~{name}(
                    __rustier_env_raw: usize{params}
                ) -> u32
            )
            .apply(printer),
            Backend::Node => frag!(
                pub unsafe extern "C" fn ~{name}(
                    __rustier_env: ::runtime_next::transport::node::Env,
                    __rustier_info: ::runtime_next::transport::node::CallbackInfo
                ) -> ::runtime_next::transport::node::Value
            )
            .apply(printer),
        });
        let enter = fragment(|printer| {
            if backend == Backend::Node {
                let count = arguments.clone().count();

                frag!(
                    {"let __rustier_call = |__rustier_env_raw: usize, __rustier_values: [usize; "}
                    {count}
                    {"]| {\n"}
                )
                .apply(printer);
            }
        });
        let leave = fragment(|printer| {
            if backend == Backend::Node {
                let count = arguments.clone().count();

                frag!(
                    {"\n};\n"}
                    unsafe {{
                        ::runtime_next::transport::node::callback::<{count}>(
                            __rustier_env, __rustier_info, __rustier_call
                        )
                    }}
                )
                .apply(printer);
            }
        });

        frag!(
            {signature} ~{"{\n"}
            {enter}
            let __rustier_env = ::runtime_next::runtime::js::JsEnv::from(unsafe {{
                ::runtime_next::transport::JsEnvHandle::from_raw(__rustier_env_raw)
            }});@
            {owned_inputs}
            {validation}
            {conversions}
            let __rustier_output: ~{return_source} = {"(|| -> "}{return_source}{" {\n"}
            {body}
            {"\n})();\n"}
            {export}
            {leave}
            {"\n}"}
        )
        .print()
    }
}

#[salsa::tracked]
impl<'db> Module<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db, backend: Backend) -> String {
        let functions = fragment(|printer| {
            for function in self.functions(db) {
                frag!({function.render(db, backend)}@@).apply(printer);
            }
        });

        let registration = fragment(|printer| {
            if backend != Backend::Node {
                return;
            }

            let exports = fragment(|printer| {
                for function in self.functions(db) {
                    let name = function.name(db);
                    let literal = format!("{name:?}");

                    frag!(
                        ::runtime_next::transport::node::register(
                            __rustier_env, __rustier_exports,
                            {("c", literal)}, Some({name})
                        );@
                    )
                    .apply(printer);
                }
            });

            frag!(
                #[unsafe(no_mangle)]@
                pub extern "C" fn node_api_module_get_api_version_v1() -> i32 {{
                    10
                }}@@
                #[unsafe(no_mangle)]@
                pub unsafe extern "C" fn napi_register_module_v1(
                    __rustier_env: ::runtime_next::transport::node::Env,
                    __rustier_exports: ::runtime_next::transport::node::Value
                ) -> ::runtime_next::transport::node::Value {{
                    ::runtime_next::transport::node::initialize();@
                    unsafe {{
                        ::runtime_next::transport::node::guard(__rustier_env, || {{
                            {exports}
                            ::runtime_next::transport::node::register(
                                __rustier_env, __rustier_exports,
                                c"__rustier_rust_drop",
                                Some(::runtime_next::transport::node::drop_rust_handle)
                            );@
                            __rustier_exports as usize
                        }})
                    }}
                }}
            )
            .apply(printer);
        });

        frag!({functions}{registration}).print()
    }
}
