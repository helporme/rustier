//! Node transport backend.

use crate::{
    codegen::fragment::{FragmentOnce, FragmentPrint, frag, fragment},
    db::Db,
    ir::transport::{TransportFunction, TransportTypeRs},
};

/// Native Node-API addon transport configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TransportBackendNode;

pub(super) fn render_transport_preamble_js(
    _node: &TransportBackendNode,
    bin_file_path: &String,
) -> impl FragmentOnce {
    // Debug formatting escapes quotes, backslashes, and control characters.
    let path = format!("{bin_file_path:?}");
    frag!(
        import {{ createRequire as __rustier_create_require }} from "node:module";@
        const __rustier_exports = __rustier_create_require(import.meta.url)({path});@
    )
}

pub(super) fn render_transport_preamble_rs() -> impl FragmentOnce {
    frag!(
        #[allow(unused_imports)]@
        use ::rustier_runtime::transport::{{JsEnvHandle as __RustierEnvHandle, JsValueHandle as __RustierValueHandle, napi_sys as __rustier_napi}};@
    )
}

pub(super) fn render_registration<'db>(
    db: &'db dyn Db,
    bind_imports: Option<TransportFunction<'db>>,
    functions: impl IntoIterator<Item = TransportFunction<'db>>,
) -> String {
    let state = match bind_imports {
        Some(_) => frag!(
            let Ok(data) = (unsafe {{ __RustierEnvHandle::allocate_static_data(env, __RustierBridgeState::new()) }}) else {{
                unsafe {{ __rustier_napi::napi_throw_error(env, ::std::ptr::null(), c"Could not allocate Rustier bridge state".as_ptr()); }}@
                return ::std::ptr::null_mut();@
            }};@
        ).print(),
        None => frag!(let data = ::std::ptr::null_mut();@).print(),
    };
    let exports = fragment(|p| {
        for function in functions.into_iter().chain(bind_imports) {
            let name = function.symbol(db);
            let name_literal = format!("c{name:?}");
            let name_literal = name_literal.as_str();
            let length = name.len();
            frag!(
                let mut function = ::std::ptr::null_mut();@
                let status = unsafe {{
                    __rustier_napi::napi_create_function(env, {name_literal}.as_ptr(), {length}, Some({name}), data, &mut function)
                }};@
                if status != __rustier_napi::Status::napi_ok {{
                    unsafe {{ __rustier_napi::napi_throw_error(env, ::std::ptr::null(), c"Could not create Rustier function".as_ptr()); }}@
                    return ::std::ptr::null_mut();@
                }}@
                let status = unsafe {{
                    __rustier_napi::napi_set_named_property(env, exports, {name_literal}.as_ptr(), function)
                }};@
                if status != __rustier_napi::Status::napi_ok {{
                    unsafe {{ __rustier_napi::napi_throw_error(env, ::std::ptr::null(), c"Could not export Rustier function".as_ptr()); }}@
                    return ::std::ptr::null_mut();@
                }}@
            ).apply(p);
        }
    });
    frag!(
        #[unsafe(no_mangle)]@
        pub unsafe extern "C" fn napi_register_module_v1(
            env: __rustier_napi::napi_env,
            exports: __rustier_napi::napi_value
        ) -> __rustier_napi::napi_value {{
            #[cfg(target_env = "msvc")]@
            {{
                static SETUP: ::std::sync::Once = ::std::sync::Once::new();@
                SETUP.call_once(|| ::std::mem::forget(unsafe {{ __rustier_napi::setup() }}));@
            }}@
            {state}
            {exports}
            exports
        }}
    )
    .print()
}

impl TransportBackendNode {
    pub(super) fn render_rs_adapter_with_body<'db>(
        &self,
        function: TransportFunction<'db>,
        db: &'db dyn Db,
        render_body: impl FnOnce(&str, &[String]) -> String,
    ) -> String {
        let name = function.symbol(db);
        let count = function.inputs(db).len();
        let arguments: Vec<_> = (0..count)
            .map(|index| format!("__rustier_handle_{index}"))
            .collect();
        let body = render_body("__rustier_env_handle", &arguments);
        let unwrap = function.can_return_error(db).then(|| {
            frag!(
                let __rustier_output = match __rustier_output {{
                    Ok(value) => value,
                    Err(error) => {{
                        __rustier_env_handle.throw_error(error);@
                        return ::std::ptr::null_mut();@
                    }}
                }};@
            )
        });
        let conversions = fragment(|p| {
            for (index, type_) in function.inputs(db).iter().enumerate() {
                match type_ {
                    TransportTypeRs::String => frag!(
                        let __rustier_handle_{index} = unsafe {{
                            __RustierValueHandle::from_napi_value(__rustier_args[{index}] as usize, __rustier_env_handle)
                        }}.to_string().expect("Expected a JavaScript string");@
                    ).apply(p),
                    TransportTypeRs::Bool => frag!(
                        let mut __rustier_handle_{index}: bool = false;@
                        let status = unsafe {{
                            __rustier_napi::napi_get_value_bool(env, __rustier_args[{index}], &mut __rustier_handle_{index})
                        }};@
                        if status != __rustier_napi::Status::napi_ok {{
                            panic!("Expected a JavaScript boolean");@
                        }}
                    ).apply(p),
                    TransportTypeRs::Void => frag!(
                        let __rustier_handle_{index} = ();@
                    ).apply(p),
                    TransportTypeRs::I128 | TransportTypeRs::U128 => frag!(
                        let __rustier_handle_{index} = unsafe {{
                            __RustierValueHandle::from_napi_value(__rustier_args[{index}] as usize, __rustier_env_handle)
                        }}.as_{type_.rust_name()}().expect("Expected a validated bigint");@
                    ).apply(p),
                    TransportTypeRs::Usize | TransportTypeRs::I64 | TransportTypeRs::U64 => {
                        let suffix = if *type_ == TransportTypeRs::I64 { "int64" } else { "uint64" };
                        let target = type_.rust_name();
                        let error = format!("c\"Rustier input {index}: bigint outside {target} range\"");
                        frag!(
                            let mut raw = 0;@
                            let mut lossless = false;@
                            let status = unsafe {{
                                __rustier_napi::napi_get_value_bigint_{suffix}(env, __rustier_args[{index}], &mut raw, &mut lossless)
                            }};@
                            let (__rustier_napi::Status::napi_ok, true, Ok(__rustier_handle_{index})) = (status, lossless, {target}::try_from(raw)) else {{
                                unsafe {{ __rustier_napi::napi_throw_type_error(env, ::std::ptr::null(), {error}.as_ptr()); }}@
                                return ::std::ptr::null_mut();@
                            }};@
                        ).apply(p);
                    }
                    TransportTypeRs::F64 => frag!(
                        let mut __rustier_handle_{index}: f64 = 0.0;@
                        let status = unsafe {{
                            __rustier_napi::napi_get_value_double(env, __rustier_args[{index}], &mut __rustier_handle_{index})
                        }};@
                        if status != __rustier_napi::Status::napi_ok {{
                            panic!("Expected a JavaScript number");@
                        }}
                    ).apply(p),
                    TransportTypeRs::JsValueHandle => frag!(
                        let __rustier_handle_{index} = unsafe {{
                            __RustierValueHandle::from_napi_value(__rustier_args[{index}] as usize, __rustier_env_handle)
                        }};@
                    ).apply(p),
                }
            }
        });
        let output = fragment(|p| {
            match function.ret_type(db) {
            TransportTypeRs::String => frag!(unsafe {
                {
                    __rustier_env_handle
                        .new_string(&__rustier_output)
                        .into_napi_value() as __rustier_napi::napi_value
                }
            })
            .apply(p),
            TransportTypeRs::Bool => frag!(
                {{
                    let mut value = ::std::ptr::null_mut();@
                    let status = unsafe {{
                        __rustier_napi::napi_get_boolean(env, __rustier_output, &mut value)
                    }};@@
                    if status != __rustier_napi::Status::napi_ok {{
                        panic!("Could not create a JavaScript boolean");@
                    }}@
                    value
                }}
            )
            .apply(p),
            TransportTypeRs::Void => frag!({{
                let mut undefined = ::std::ptr::null_mut();@
                let status = unsafe {{ __rustier_napi::napi_get_undefined(env, &mut undefined) }};@
                if status != __rustier_napi::Status::napi_ok {{
                    unsafe {{ __rustier_napi::napi_throw_error(env, ::std::ptr::null(), c"Could not obtain undefined".as_ptr()); }}@
                    return ::std::ptr::null_mut();@
                }}@
                undefined
            }}).apply(p),
            kind @ (TransportTypeRs::I128 | TransportTypeRs::U128) => frag!(
                unsafe {{ __rustier_env_handle.new_{kind.rust_name()}(__rustier_output).into_napi_value() as __rustier_napi::napi_value }}
            ).apply(p),
            kind @ (TransportTypeRs::Usize | TransportTypeRs::I64 | TransportTypeRs::U64) => {
                let (suffix, raw) = if kind == TransportTypeRs::I64 { ("int64", "i64") } else { ("uint64", "u64") };
                frag!(
                    {{
                        let mut value = ::std::ptr::null_mut();@
                        let status = unsafe {{
                            __rustier_napi::napi_create_bigint_{suffix}(env, __rustier_output as ~{raw}, &mut value)
                        }};@
                        if status != __rustier_napi::Status::napi_ok {{
                            panic!("Could not create a JavaScript bigint");@
                        }}@
                        value
                    }}
                ).apply(p);
            }
            TransportTypeRs::F64 => frag!(
                {{
                    let mut value = ::std::ptr::null_mut();@
                    let status = unsafe {{
                        __rustier_napi::napi_create_double(env, __rustier_output, &mut value)
                    }};@@
                    if status != __rustier_napi::Status::napi_ok {{
                        panic!("Could not create a JavaScript number");@
                    }}@
                    value
                }}
            )
            .apply(p),
            TransportTypeRs::JsValueHandle => frag!(unsafe {
                { __rustier_output.into_napi_value() as __rustier_napi::napi_value }
            })
            .apply(p),
        }
        });
        frag!(
            unsafe extern "C" fn ~{name}(
                env: __rustier_napi::napi_env,
                info: __rustier_napi::napi_callback_info
            ) -> __rustier_napi::napi_value {{
                let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {{
                    let mut __rustier_args = [::std::ptr::null_mut(); {count}];@
                    let mut argc = {count};@
                    let mut data = ::std::ptr::null_mut();@
                    let status = unsafe {{
                        __rustier_napi::napi_get_cb_info(env, info, &mut argc, __rustier_args.as_mut_ptr(), ::std::ptr::null_mut(), &mut data)
                    }};@
                    if status != __rustier_napi::Status::napi_ok {{
                        unsafe {{ __rustier_napi::napi_throw_error(env, ::std::ptr::null(), c"Could not read Rustier callback info".as_ptr()); }}@
                        return ::std::ptr::null_mut();@
                    }}@
                    let __rustier_env_handle = unsafe {{ __RustierEnvHandle::from_raw(env, data) }};@
                    {conversions}
                    let __rustier_output = {"{\n"}{body}{"\n};"}@
                    {unwrap}
                    {output}
                }}));@
                match result {{
                    Ok(value) => value,
                    Err(_) => {{
                        unsafe {{ __rustier_napi::napi_throw_error(env, ::std::ptr::null(), c"Rust macro panicked".as_ptr()); }}@
                        ::std::ptr::null_mut()
                    }}
                }}
            }}
        ).print()
    }

    pub(super) fn render_js_ffi_call_stmt<'db>(
        &self,
        function: TransportFunction<'db>,
        db: &'db dyn Db,
        arguments: &[String],
        result_var: &str,
    ) -> String {
        let name = function.symbol(db);
        let arguments = arguments.join(", ");
        frag!({result_var} = __rustier_exports.{name}{"("}{arguments}{");"}).print()
    }
}
