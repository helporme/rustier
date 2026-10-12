//! All platform-specific code generation utilities.

use crate::compiler::codegen::{
    fragment::{FragmentOnce, doubles, frag, fragment},
    runtime::Platform,
};

// todo: split it on multiple files?

impl Platform {
    /// The prelude code for this platform.
    pub fn prelude(&self) -> &str {
        match self {
            Platform::Wasm { .. } => "use wasm_bindgen::prelude::*;\n",
            Platform::Node => "use napi_derive::napi;\n",
        }
    }

    pub fn header_renamed<'a>(self, name: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        let platform_attr = match self {
            Platform::Wasm { .. } => "wasm_bindgen",
            Platform::Node => "napi",
        };

        frag!(
            #[{platform_attr}(js_name = {doubles(name)})]
        )
    }

    pub fn header(self) -> impl FragmentOnce {
        let platform_attr = match self {
            Platform::Wasm { .. } => "wasm_bindgen",
            Platform::Node => "napi",
        };

        frag!(
            #[{platform_attr}]
        )
    }

    /// The getter header for a field with the given JavaScript name.
    pub fn field_getter_header<'a>(self, name: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        let platform_attr = match self {
            Platform::Wasm { .. } => "wasm_bindgen",
            Platform::Node => "napi",
        };

        frag!(
            #[{platform_attr}(getter, js_name = {doubles(name)})]
        )
    }

    /// The setter header for a field with the given JavaScript name.
    pub fn field_setter_header<'a>(self, name: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        let platform_attr = match self {
            Platform::Wasm { .. } => "wasm_bindgen",
            Platform::Node => "napi",
        };

        frag!(
            #[{platform_attr}(setter, js_name = {doubles(name)})]
        )
    }

    /// The error type for this platform.
    pub fn result_type<'a>(self, ok_type: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        fragment(move |p| match self {
            Platform::Wasm { .. } => frag!(Result<{ok_type}, JsValue>).apply(p),
            Platform::Node => frag!(napi::Result<{ ok_type }>).apply(p),
        })
    }

    /// Converts a runtime error into this platform's boundary error.
    pub fn error<'a>(self, error: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        fragment(move |p| match self {
            Platform::Wasm { .. } => frag!(JsValue::from_str(&{ error }.to_string())).apply(p),
            Platform::Node => frag!(napi::Error::from_reason({ error }.to_string())).apply(p),
        })
    }

    /// The contents of `fn main` for this platform's build script, if
    /// any.
    pub const fn build_script_body(&self) -> Option<&'static str> {
        match self {
            Platform::Wasm => None,
            Platform::Node => Some("napi_build::setup();"),
        }
    }
}
