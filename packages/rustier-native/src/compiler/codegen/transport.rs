//! Code generation for the ABI between the Rust runtime and JavaScript.

use crate::compiler::{
    codegen::fragment::{FragmentOnce, frag, fragment},
    db::Db,
    ir::transport::{Transport, backend::TransportBackend},
};

pub mod js;
mod rust;

pub use rust::{TransportArg, TransportFunctionRenderOps, TransportInput};

impl TransportBackend {
    pub fn render_dependencies(self) -> impl FragmentOnce {
        fragment(move |p| match self {
            Self::Wasm(_) => frag!(wasm-bindgen~=~"0.2").apply(p),
            Self::Napi => frag!(
                napi = ~{"{"}~version = "3", features = ~["napi10"]~{"}"}@
                napi-derive = ~{"{"}~version = "3"~{"}"}
            )
            .apply(p),
        })
    }

    pub fn render_build_dependencies(self) -> Option<impl FragmentOnce> {
        matches!(self, Self::Napi).then_some(frag!(
            @@
            [build-dependencies]@
            napi-build = "2"
        ))
    }

    /// The prelude code for this transport.
    pub fn prelude(self) -> &'static str {
        match self {
            TransportBackend::Wasm(_) => "use wasm_bindgen::prelude::*;\n",
            TransportBackend::Napi => "use napi_derive::napi;\n",
        }
    }

    /// The contents of `fn main` for this transport's build script, if any.
    pub const fn build_script_body(self) -> Option<&'static str> {
        match self {
            TransportBackend::Wasm(_) => None,
            TransportBackend::Napi => Some("napi_build::setup();"),
        }
    }
}

impl<'db> Transport<'db> {
    pub fn render_preamble(
        self,
        db: &'db dyn Db,
        backend: TransportBackend,
    ) -> impl FragmentOnce + 'db {
        let node_api_version = matches!(backend, TransportBackend::Napi).then_some(frag!(
            #[unsafe(no_mangle)]@
            pub extern "C" fn node_api_module_get_api_version_v1()~ -> i32 {{
                10
            }}@@
        ));
        let adapter = (matches!(backend, TransportBackend::Napi)
            && self
                .facade(db)
                .runtime(db)
                .functions(db)
                .iter()
                .any(|function| function.transport(db).uses_js_values(db)))
        .then(render_js_value_adapter);

        frag!({backend.prelude()}@@{node_api_version}{adapter}@@)
    }
}

fn render_js_value_adapter() -> impl FragmentOnce {
    frag!(
        pub struct __JsValue(pub rustier_ext::transport::JsValueHandle);@@

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
                    rustier_ext::transport::JsEnvHandle::from_raw(env as usize)
                }};@
                let handle = unsafe {{
                    rustier_ext::transport::JsValueHandle::from_napi_value(value as usize, env)
                }};@@

                Ok(Self(handle))
            }}
        }}@@

        impl napi::bindgen_prelude::ToNapiValue for __JsValue {{
            unsafe fn to_napi_value(
                env: napi::sys::napi_env,
                value: Self
            )~ -> napi::Result<napi::sys::napi_value> {{
                let handle = value.0;@@

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
