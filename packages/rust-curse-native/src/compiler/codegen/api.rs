//! All api-specific code generation utilities.

use crate::compiler::{
    codegen::fragment::{FragmentOnce, doubles, frag, fragment, singles},
    inputs::{Api, WasmTarget},
};

// todo: split it on multiple files?

impl Api {
    /// Returns the prelude code for this API.
    pub fn prelude(&self) -> &str {
        match self {
            Api::Wasm { .. } => "use wasm_bindgen::prelude::*;\n",
            Api::Node => "use napi::{Unknown, ValueType};\nuse napi_derive::napi;\n",
        }
    }

    /// Returns the Rust boundary type used for incoming JavaScript values.
    pub fn js_type(&self) -> &'static str {
        match self {
            Api::Wasm { .. } => "JsValue",
            Api::Node => "Unknown<'_>",
        }
    }

    /// Returns an API-error expression for casting a JavaScript value to
    /// `bool`.
    pub fn cast_to_bool(self) -> impl FragmentOnce {
        self.cast_to_primitive("bool", "as_bool", "Boolean")
    }

    /// Returns an API-error expression for casting a JavaScript value to
    /// `String`.
    pub fn cast_to_string(self) -> impl FragmentOnce {
        self.cast_to_primitive("String", "as_string", "String")
    }

    /// Returns an API-error expression for casting a JavaScript value to `f64`.
    pub fn cast_to_f64(self) -> impl FragmentOnce {
        self.cast_to_primitive("f64", "as_f64", "Number")
    }

    /// Returns a string-error expression for the legacy JavaScript-to-`bool`
    /// cast function.
    pub fn cast_to_bool_with_string_error(self) -> impl FragmentOnce {
        self.cast_to_primitive_with_string_error("bool", "as_bool", "Boolean")
    }

    /// Returns a string-error expression for the legacy JavaScript-to-`String`
    /// cast function.
    pub fn cast_to_string_with_string_error(self) -> impl FragmentOnce {
        self.cast_to_primitive_with_string_error("String", "as_string", "String")
    }

    /// Returns a string-error expression for the legacy JavaScript-to-`f64`
    /// cast function.
    pub fn cast_to_f64_with_string_error(self) -> impl FragmentOnce {
        self.cast_to_primitive_with_string_error("f64", "as_f64", "Number")
    }

    fn cast_to_primitive(
        self,
        rust_type: &'static str,
        wasm_accessor: &'static str,
        node_value_type: &'static str,
    ) -> impl FragmentOnce {
        let wasm_error =
            self.error(frag!({ doubles(("cannot cast value to ", rust_type)) }.to_string()));
        let node_get_type_error = self.error("err.reason");
        let node_cast_error =
            self.error(frag!({ doubles(("cannot cast value to ", rust_type)) }.to_string()));
        let node_value_type_error =
            self.error(frag!({ doubles(("cannot cast value to ", rust_type)) }.to_string()));

        fragment(move |p| {
            match self {
            Api::Wasm { .. } => frag!(
                value.{wasm_accessor}().ok_or_else(||~{wasm_error})
            )
            .apply(p),
            Api::Node => frag!(
                if value.get_type().map_err(|err|~{node_get_type_error})?~ == ValueType::{node_value_type}~{{
                    unsafe~{{
                        value.cast::<{rust_type}>()
                    }}.map_err(|_|~{node_cast_error})
                }}@
                else~{{
                    Err({node_value_type_error})
                }}
            )
            .apply(p),
        }
        })
    }

    fn cast_to_primitive_with_string_error(
        self,
        rust_type: &'static str,
        wasm_accessor: &'static str,
        node_value_type: &'static str,
    ) -> impl FragmentOnce {
        fragment(move |p| match self {
            Api::Wasm { .. } => frag!(
                value.{wasm_accessor}().ok_or_else(||~
                    {doubles(("cannot cast value to ", rust_type))}.to_string()
                )
            )
            .apply(p),
            Api::Node => frag!(
                if value.get_type().map_err(|err| err.reason)?~ == ValueType::{node_value_type}~{{
                    unsafe~{{
                        value.cast::<{rust_type}>()
                    }}.map_err(|_|~{doubles(("cannot cast value to ", rust_type))}.to_string())
                }}@
                else~{{
                    Err({doubles(("cannot cast value to ", rust_type))}.to_string())
                }}
            )
            .apply(p),
        })
    }

    pub fn header_renamed<'a>(
        self,
        name: impl FragmentOnce + 'a,
    ) -> impl FragmentOnce + 'a {
        let api_name = match self {
            Api::Wasm { .. } => "wasm_bindgen",
            Api::Node => "napi",
        };

        frag!(
            #[{api_name}(js_name = {doubles(name)})]
        )
    }

    pub fn header(self) -> impl FragmentOnce {
        let api_name = match self {
            Api::Wasm { .. } => "wasm_bindgen",
            Api::Node => "napi",
        };

        frag!(
            #[{api_name}]
        )
    }

    /// Returns the getter header for a field with the given JavaScript name.
    pub fn field_getter_header<'a>(
        self,
        name: impl FragmentOnce + 'a,
    ) -> impl FragmentOnce + 'a {
        let api_name = match self {
            Api::Wasm { .. } => "wasm_bindgen",
            Api::Node => "napi",
        };

        frag!(
            #[{api_name}(getter, js_name = {doubles(name)})]
        )
    }

    /// Returns the setter header for a field with the given JavaScript name.
    pub fn field_setter_header<'a>(
        self,
        name: impl FragmentOnce + 'a,
    ) -> impl FragmentOnce + 'a {
        let api_name = match self {
            Api::Wasm { .. } => "wasm_bindgen",
            Api::Node => "napi",
        };

        frag!(
            #[{api_name}(setter, js_name = {doubles(name)})]
        )
    }

    /// Returns the error type for this API.
    pub fn result_type<'a>(self, ok_type: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        fragment(move |p| match self {
            Api::Wasm { .. } => frag!(Result<{ok_type}, JsValue>).apply(p),
            Api::Node => frag!(napi::Result<{ ok_type }>).apply(p),
        })
    }

    /// Returns an error expression with the given reason.
    pub fn error<'a>(self, reason: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        fragment(move |p| match self {
            Api::Wasm { .. } => frag!(JsValue::from_str(&{ reason })).apply(p),
            Api::Node => frag!(napi::Error::from_reason({ reason })).apply(p),
        })
    }

    /// Returns the contents of `fn main` for this API's build script, if any.
    pub fn build_script_body(&self) -> Option<&'static str> {
        match self {
            Api::Wasm { .. } => None,
            Api::Node => Some("napi_build::setup();"),
        }
    }

    /// Returns whether this API needs a generated Cargo build script.
    pub fn needs_build_script(&self) -> bool {
        self.build_script_body().is_some()
    }

    /// Returns the Cargo manifest dependencies required by this API.
    pub fn manifest_deps(self) -> Option<impl FragmentOnce> {
        Some(fragment(move |p| match self {
            Api::Wasm { .. } => frag!({ r#"wasm-bindgen = "0.2""# }).apply(p),
            Api::Node => frag!(
                {r#"napi = { version = "3", features = ["napi10"] }"#}@
                {r#"napi-derive = { version = "3" }"#}
            )
            .apply(p),
        }))
    }

    /// Returns the Cargo manifest build dependencies required by this API.
    pub fn manifest_build_deps(self) -> Option<impl FragmentOnce> {
        let deps = frag!(
            @@
            [build-dependencies]@
            {r#"napi-build = "2""#}
        );

        (self == Api::Node).then_some(deps)
    }

    /// Returns JavaScript initialization code needed by this API before exports
    /// are used.
    pub fn js_mod_init<'a>(self, src_path: &'a str) -> Option<impl FragmentOnce + 'a> {
        matches!(
            self,
            Api::Wasm {
                target: WasmTarget::Web
            }
        )
        .then(|| {
            frag!(
                import init from ~{singles(src_path)};@
                await init();
            )
        })
    }
}
