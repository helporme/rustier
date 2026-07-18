//! All api-specific code generation utilities.

use salsa::Setter;

use crate::codegen::{
    db::Db,
    fragment::{FragmentOnce, doubles, frag, fragment, singles},
    ir::{
        fn_::Fn,
        struct_::{Struct, StructField},
    },
};

/// Active binding API used for generated Rust and JavaScript glue code.
#[salsa::input(singleton)]
pub struct Api {
    #[returns(copy)]
    pub kind: ApiKind,
}

/// Binding backend selected for code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiKind {
    /// wasm-bindgen
    Wasm { target: WasmTarget },
    /// napi
    Node,
}

/// JavaScript platform targeted by wasm-bindgen bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WasmTarget {
    Web,
    Node,
}

// todo: split it on multiple files?

impl Api {
    /// Sets the active binding API in the database.
    pub fn set(db: &mut dyn Db, kind: ApiKind) -> Self {
        if let Some(api) = Self::try_get(db) {
            api.set_kind(db).to(kind);
            api
        } else {
            Self::new(db, kind)
        }
    }

    /// Returns the prelude code for this API.
    pub fn prelude(&self, db: &dyn Db) -> &str {
        match self.kind(db) {
            ApiKind::Wasm { .. } => "use wasm_bindgen::prelude::*;\n",
            ApiKind::Node => "use napi_derive::napi;\n",
        }
    }

    /// Returns extra prelude code required by generated cast functions.
    pub fn cast_prelude(&self, db: &dyn Db) -> Option<&'static str> {
        match self.kind(db) {
            ApiKind::Wasm { .. } => None,
            ApiKind::Node => Some("use napi::bindgen_prelude::{Unknown, ValueType};\n"),
        }
    }

    /// Returns the Rust boundary type used for incoming JavaScript values.
    pub fn js_type(&self, db: &dyn Db) -> &'static str {
        match self.kind(db) {
            ApiKind::Wasm { .. } => "JsValue",
            ApiKind::Node => "Unknown<'_>",
        }
    }

    /// Returns the API-specific body for casting a JavaScript value to `bool`.
    ///
    /// The expression must be inserted into a scope returning this API's result
    /// type because some backends generate `?` operations.
    pub fn cast_to_bool(self, db: &dyn Db) -> impl FragmentOnce {
        self.cast_to_primitive(db, "bool", "as_bool", "Boolean")
    }

    /// Returns the API-specific body for casting a JavaScript value to
    /// `String`.
    ///
    /// The expression must be inserted into a scope returning this API's result
    /// type because some backends generate `?` operations.
    pub fn cast_to_string(self, db: &dyn Db) -> impl FragmentOnce {
        self.cast_to_primitive(db, "String", "as_string", "String")
    }

    /// Returns the API-specific body for casting a JavaScript value to `f64`.
    ///
    /// The expression must be inserted into a scope returning this API's result
    /// type because some backends generate `?` operations.
    pub fn cast_to_f64(self, db: &dyn Db) -> impl FragmentOnce {
        self.cast_to_primitive(db, "f64", "as_f64", "Number")
    }

    fn cast_to_primitive(
        self,
        db: &dyn Db,
        rust_type: &'static str,
        wasm_accessor: &'static str,
        node_value_type: &'static str,
    ) -> impl FragmentOnce {
        let kind = self.kind(db);

        fragment(move |p| match kind {
            ApiKind::Wasm { .. } => frag!(
                value.{wasm_accessor}().ok_or_else(||~
                    {doubles(("cannot cast value to ", rust_type))}.to_string()
                )
            )
            .apply(p),
            ApiKind::Node => frag!(
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

    /// Returns the function header for this API.
    pub fn fn_header<'db>(self, db: &'db dyn Db, _fn: Fn<'db>) -> impl FragmentOnce + 'db {
        let header = match self.kind(db) {
            ApiKind::Wasm { .. } => "#[wasm_bindgen(js_name = ",
            ApiKind::Node => "#[napi(js_name = ",
        };

        frag!({header}{doubles(_fn.bindings_name(db))}{")]"})
    }

    pub fn impl_header(&self, db: &dyn Db) -> &'static str {
        match self.kind(db) {
            ApiKind::Wasm { .. } => "#[wasm_bindgen]",
            ApiKind::Node => "#[napi]",
        }
    }

    /// Returns the struct header for this API.
    pub fn struct_header<'db>(
        self,
        db: &'db dyn Db,
        _struct: Struct<'db>,
    ) -> impl FragmentOnce + 'db {
        let header = match self.kind(db) {
            ApiKind::Wasm { .. } => "#[wasm_bindgen(js_name = ",
            ApiKind::Node => "#[napi(js_name = ",
        };

        frag!({header}{doubles(_struct.binding_name(db))}{")]"})
    }

    /// Returns the getter header for this API.
    pub fn field_getter_header<'db>(
        self,
        db: &'db dyn Db,
        field: StructField<'db>,
    ) -> impl FragmentOnce + 'db {
        let header = match self.kind(db) {
            ApiKind::Wasm { .. } => "#[wasm_bindgen(getter, js_name = ",
            ApiKind::Node => "#[napi(getter, js_name = ",
        };

        frag!({header}{doubles(field.name(db))}{")]"})
    }

    /// Returns the setter header for this API.
    pub fn field_setter_header<'db>(
        self,
        db: &'db dyn Db,
        field: StructField<'db>,
    ) -> impl FragmentOnce + 'db {
        let header = match self.kind(db) {
            ApiKind::Wasm { .. } => "#[wasm_bindgen(setter, js_name = ",
            ApiKind::Node => "#[napi(setter, js_name = ",
        };

        frag!({header}{doubles(field.name(db))}{")]"})
    }

    /// Returns the error type for this API.
    pub fn result_type<'a>(
        self,
        db: &dyn Db,
        ok_type: impl FragmentOnce + 'a,
    ) -> impl FragmentOnce + 'a {
        let kind = self.kind(db);

        fragment(move |p| match kind {
            ApiKind::Wasm { .. } => frag!(Result<{ok_type}, JsValue>).apply(p),
            ApiKind::Node => frag!(napi::Result<{ ok_type }>).apply(p),
        })
    }

    /// Returns an error expression with the given reason.
    pub fn error<'a>(self, db: &dyn Db, reason: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        let kind = self.kind(db);

        fragment(move |p| match kind {
            ApiKind::Wasm { .. } => frag!(JsValue::from_str(&{ reason })).apply(p),
            ApiKind::Node => frag!(napi::Error::from_reason({ reason })).apply(p),
        })
    }

    /// Returns the contents of `fn main` for this API's build script, if any.
    pub fn build_script_body(&self, db: &dyn Db) -> Option<&'static str> {
        match self.kind(db) {
            ApiKind::Wasm { .. } => None,
            ApiKind::Node => Some("napi_build::setup();"),
        }
    }

    /// Returns whether this API needs a generated Cargo build script.
    pub fn needs_build_script(&self, db: &dyn Db) -> bool {
        self.build_script_body(db).is_some()
    }

    /// Returns the Cargo manifest dependencies required by this API.
    pub fn manifest_deps(self, db: &dyn Db) -> Option<impl FragmentOnce> {
        let kind = self.kind(db);

        Some(fragment(move |p| match kind {
            ApiKind::Wasm { .. } => frag!({ r#"wasm-bindgen = "0.2""# }).apply(p),
            ApiKind::Node => frag!(
                {r#"napi = { version = "3", features = ["napi10"] }"#}@
                {r#"napi-derive = { version = "3" }"#}
            )
            .apply(p),
        }))
    }

    /// Returns the Cargo manifest build dependencies required by this API.
    pub fn manifest_build_deps(self, db: &dyn Db) -> Option<impl FragmentOnce> {
        let deps = frag!(
            @@
            [build-dependencies]@
            {r#"napi-build = "2""#}
        );

        (self.kind(db) == ApiKind::Node).then_some(deps)
    }

    /// Returns JavaScript initialization code needed by this API before exports
    /// are used.
    pub fn js_mod_init<'a>(self, db: &dyn Db, src_path: &'a str) -> Option<impl FragmentOnce + 'a> {
        matches!(
            self.kind(db),
            ApiKind::Wasm {
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
