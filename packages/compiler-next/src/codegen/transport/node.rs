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
        use ::aux_next::transport::{{JsEnvHandle as __RustierEnvHandle, JsValueHandle as __RustierValueHandle, napi_sys as __rustier_napi}};@
    )
}

pub(super) fn render_registration<'db>(
    db: &'db dyn Db,
    functions: impl IntoIterator<Item = TransportFunction<'db>>,
) -> String {
    let exports = fragment(|p| {
        for function in functions {
            let name = function.symbol(db);
            let name_literal = format!("c{name:?}");
            let name_literal = name_literal.as_str();
            let length = name.len();
            frag!(
                let mut function = ::std::ptr::null_mut();@
                assert_eq!(unsafe {{
                    __rustier_napi::napi_create_function(env, {name_literal}.as_ptr(), {length}, Some({name}), ::std::ptr::null_mut(), &mut function)
                }}, __rustier_napi::Status::napi_ok);@
                assert_eq!(unsafe {{
                    __rustier_napi::napi_set_named_property(env, exports, {name_literal}.as_ptr(), function)
                }}, __rustier_napi::Status::napi_ok);@
            ).apply(p);
        }
    });
    frag!(
        #[unsafe(no_mangle)]@
        pub unsafe extern "C" fn napi_register_module_v1(
            env: __rustier_napi::napi_env,
            exports: __rustier_napi::napi_value
        ) -> __rustier_napi::napi_value {{
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
        let output = fragment(|p| match function.ret_type(db) {
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
            TransportTypeRs::Void => frag!(undefined).apply(p),
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
        });
        frag!(
            unsafe extern "C" fn ~{name}(
                env: __rustier_napi::napi_env,
                info: __rustier_napi::napi_callback_info
            ) -> __rustier_napi::napi_value {{
                let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {{
                    let mut __rustier_args = [::std::ptr::null_mut(); {count}];@
                    let mut argc = {count};@
                    assert_eq!(unsafe {{
                        __rustier_napi::napi_get_cb_info(env, info, &mut argc, __rustier_args.as_mut_ptr(), ::std::ptr::null_mut(), ::std::ptr::null_mut())
                    }}, __rustier_napi::Status::napi_ok);@
                    let mut undefined = ::std::ptr::null_mut();@
                    assert_eq!(unsafe {{ __rustier_napi::napi_get_undefined(env, &mut undefined) }}, __rustier_napi::Status::napi_ok);@
                    for argument in __rustier_args.iter_mut().skip(argc) {{
                        *argument = undefined;@
                    }}
                    let __rustier_env_handle = unsafe {{ __RustierEnvHandle::from_raw(env as usize) }};@
                    {conversions}
                    let __rustier_output = {"{\n"}{body}{"\n};"}@
                    {output}
                }}));@
                match result {{
                    Ok(value) => value,
                    Err(_) => {{
                        unsafe {{ __rustier_napi::napi_throw_error(env, ::std::ptr::null(), c"Rust snippet panicked".as_ptr()); }}@
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
