//! All platform-specific code generation utilities.

use crate::compiler::codegen::{
    fragment::{FragmentOnce, FragmentPrint, doubles, frag, fragment, singles},
    shim::{Platform, WasmEnv},
};

// todo: split it on multiple files?

impl Platform {
    /// The prelude code for this platform.
    pub fn prelude(&self) -> &str {
        match self {
            Platform::Wasm { .. } => "use wasm_bindgen::prelude::*;\n",
            Platform::Node => "use napi::{Unknown, ValueType};\nuse napi_derive::napi;\n",
        }
    }

    /// The Rust boundary type used for incoming JavaScript values.
    pub fn js_type(&self) -> &'static str {
        match self {
            Platform::Wasm { .. } => "JsValue",
            Platform::Node => "Unknown<'_>",
        }
    }

    /// A platform-specific error expression for casting a JavaScript
    /// value to `bool`.
    pub fn cast_to_bool(self) -> impl FragmentOnce {
        self.cast_to_primitive("bool", "as_bool", "Boolean")
    }

    /// A platform-specific error expression for casting a JavaScript
    /// value to `String`.
    pub fn cast_to_string(self) -> impl FragmentOnce {
        self.cast_to_primitive("String", "as_string", "String")
    }

    /// A platform-specific error expression for casting a JavaScript
    /// value to `f64`.
    pub fn cast_to_f64(self) -> impl FragmentOnce {
        self.cast_to_primitive("f64", "as_f64", "Number")
    }

    /// A string-error expression for the legacy JavaScript-to-`bool`
    /// cast function.
    pub fn cast_to_bool_with_string_error(self) -> impl FragmentOnce {
        self.cast_to_primitive_with_string_error("bool", "as_bool", "Boolean")
    }

    /// A string-error expression for the legacy JavaScript-to-`String`
    /// cast function.
    pub fn cast_to_string_with_string_error(self) -> impl FragmentOnce {
        self.cast_to_primitive_with_string_error("String", "as_string", "String")
    }

    /// A string-error expression for the legacy JavaScript-to-`f64`
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
        let wasm_error = self.error(frag!(
            { doubles(("cannot cast value to ", rust_type)) }.to_string()
        ));
        let node_get_type_error = self.error("err.reason");
        let node_cast_error = self.error(frag!(
            { doubles(("cannot cast value to ", rust_type)) }.to_string()
        ));
        let node_value_type_error = self.error(frag!(
            { doubles(("cannot cast value to ", rust_type)) }.to_string()
        ));

        fragment(move |p| {
            match self {
            Platform::Wasm { .. } => frag!(
                value.{wasm_accessor}().ok_or_else(||~{wasm_error})
            )
            .apply(p),
            Platform::Node => frag!(
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
            Platform::Wasm { .. } => frag!(
                value.{wasm_accessor}().ok_or_else(||~
                    {doubles(("cannot cast value to ", rust_type))}.to_string()
                )
            )
            .apply(p),
            Platform::Node => frag!(
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

    /// An error expression with the given reason.
    pub fn error<'a>(self, reason: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        fragment(move |p| match self {
            Platform::Wasm { .. } => frag!(JsValue::from_str(&{ reason })).apply(p),
            Platform::Node => frag!(napi::Error::from_reason({ reason })).apply(p),
        })
    }

    /// The contents of `fn main` for this platform's build script, if
    /// any.
    pub fn build_script_body(&self) -> Option<&'static str> {
        match self {
            Platform::Wasm { .. } => None,
            Platform::Node => Some("napi_build::setup();"),
        }
    }

    /// Whether this platform needs a generated Cargo build script.
    pub fn needs_build_script(&self) -> bool {
        self.build_script_body().is_some()
    }

    /// The Cargo manifest dependencies required by this platform.
    pub fn manifest_deps(self) -> Option<impl FragmentOnce> {
        Some(fragment(move |p| match self {
            Platform::Wasm { .. } => frag!({ r#"wasm-bindgen = "0.2""# }).apply(p),
            Platform::Node => frag!(
                {r#"napi = { version = "3", features = ["napi10"] }"#}@
                {r#"napi-derive = { version = "3" }"#}
            )
            .apply(p),
        }))
    }

    /// The Cargo manifest build dependencies required by this platform.
    pub fn manifest_build_deps(self) -> Option<impl FragmentOnce> {
        let deps = frag!(
            @@
            [build-dependencies]@
            {r#"napi-build = "2""#}
        );

        (self == Platform::Node).then_some(deps)
    }

    /// Dependency entries to append to a generated manifest's `[dependencies]`
    /// section, including a `[build-dependencies]` section when needed.
    pub fn render_manifest_dependencies(self) -> String {
        frag!({self.manifest_deps()}{self.manifest_build_deps()}).print()
    }

    /// JavaScript initialization code needed by this platform before
    /// exports are used.
    pub fn js_mod_init<'a>(self, src_path: &'a str) -> Option<impl FragmentOnce + 'a> {
        matches!(
            self,
            Platform::Wasm {
                target: WasmEnv::Web
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
