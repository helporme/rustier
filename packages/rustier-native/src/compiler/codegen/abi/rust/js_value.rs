use crate::compiler::codegen::fragment::{FragmentOnce, frag};

pub fn render_js_value_adapter() -> impl FragmentOnce {
    frag!(
        pub struct __JsValue(pub rustier_rt::js::JsValue);@@

        impl napi::bindgen_prelude::TypeName for __JsValue {{
            fn type_name()~ -> &'static str {{
                "unknown"
            }}@@

            fn value_type()~ -> napi::ValueType {{
                napi::ValueType::Unknown
            }}
        }}@@

        impl napi::bindgen_prelude::ValidateNapiValue for __JsValue {{}}@@

        impl napi::bindgen_prelude::FromNapiValue for __JsValue {{
            unsafe fn from_napi_value(
                env: napi::sys::napi_env,
                value: napi::sys::napi_value
            )~ -> napi::Result<Self> {{
                let env = unsafe {{
                    rustier_rt::js::backend::EnvHandle::from_raw(env as usize)
                }};@
                let handle = unsafe {{
                    rustier_rt::js::backend::ValueHandle::from_napi_value(value as usize, env)
                }};@@

                Ok(Self(handle.into()))
            }}
        }}@@

        impl napi::bindgen_prelude::ToNapiValue for __JsValue {{
            unsafe fn to_napi_value(
                env: napi::sys::napi_env,
                value: Self
            )~ -> napi::Result<napi::sys::napi_value> {{
                let handle: rustier_rt::js::backend::ValueHandle = value.0.into();@@

                if handle.env().into_raw() != env as usize {{
                    return Err(napi::Error::new(
                        napi::Status::InvalidArg,
                        "JsValue belongs to another JavaScript environment"
                    ));
                }}@@

                Ok(unsafe {{ handle.into_napi_value() }}~ as napi::sys::napi_value)
            }}
        }}
    )
}
