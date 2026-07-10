//! All api-specific code generation utilities.

use salsa::Setter;

use crate::codegen::{
    db::Db,
    ir::{self, struct_::StructField},
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
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::{
    ///     api::{Api, ApiKind},
    ///     db::DbImpl,
    /// };
    ///
    /// let mut db = DbImpl::default();
    /// Api::set(&mut db, ApiKind::Node);
    ///
    /// assert_eq!(Api::get(&db).kind(&db), ApiKind::Node);
    /// ```
    pub fn set(db: &mut crate::codegen::db::DbImpl, kind: ApiKind) -> Self {
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
    pub fn cast_to_bool(&self, db: &dyn Db) -> String {
        self.cast_to_primitive(db, "bool", "as_bool", "Boolean")
    }

    /// Returns the API-specific body for casting a JavaScript value to
    /// `String`.
    pub fn cast_to_string(&self, db: &dyn Db) -> String {
        self.cast_to_primitive(db, "String", "as_string", "String")
    }

    /// Returns the API-specific body for casting a JavaScript value to `f64`.
    pub fn cast_to_f64(&self, db: &dyn Db) -> String {
        self.cast_to_primitive(db, "f64", "as_f64", "Number")
    }

    fn cast_to_primitive(
        &self,
        db: &dyn Db,
        rust_type: &'static str,
        wasm_accessor: &'static str,
        node_value_type: &'static str,
    ) -> String {
        match self.kind(db) {
            ApiKind::Wasm { .. } => format!(
                "value.{wasm_accessor}().ok_or_else(|| \"cannot cast value to {rust_type}\".to_string())"
            ),
            ApiKind::Node => indoc::formatdoc! {r#"
                if value.get_type().map_err(|err| err.reason)? == ValueType::{node_value_type}
                {{
                	unsafe {{ value.cast::<{rust_type}>() }}.map_err(|_| "cannot cast value to {rust_type}".to_string())
                }}
                else
                {{
                	Err("cannot cast value to {rust_type}".to_string())
                }}"#},
        }
    }

    /// Returns the function header for this API.
    pub fn fn_header(&self, db: &dyn Db, _fn: ir::fn_::Fn) -> String {
        let fn_name = _fn.bindings_name(db);
        match self.kind(db) {
            ApiKind::Wasm { .. } => format!("#[wasm_bindgen(js_name = {})]", fn_name),
            ApiKind::Node => format!("#[napi(js_name = \"{}\")]", fn_name),
        }
    }

    pub fn impl_header(&self, db: &dyn Db) -> &'static str {
        match self.kind(db) {
            ApiKind::Wasm { .. } => "#[wasm_bindgen]",
            ApiKind::Node => "#[napi]",
        }
    }

    /// Returns the struct header for this API.
    pub fn struct_header(&self, db: &dyn Db, _struct: ir::struct_::Struct) -> String {
        let struct_name = _struct.binding_name(db);
        match self.kind(db) {
            ApiKind::Wasm { .. } => format!("#[wasm_bindgen(js_name = {})]", struct_name),
            ApiKind::Node => format!("#[napi(js_name = \"{}\")]", struct_name),
        }
    }

    /// Returns the getter header for this API.
    pub fn field_getter_header(&self, db: &dyn Db, field: StructField) -> String {
        match self.kind(db) {
            ApiKind::Wasm { .. } => {
                format!("#[wasm_bindgen(getter, js_name = {})]", field.name(db))
            }
            ApiKind::Node => format!("#[napi(getter, js_name = \"{}\")]", field.name(db)),
        }
    }

    /// Returns the setter header for this API.
    pub fn field_setter_header(&self, db: &dyn Db, field: StructField) -> String {
        match self.kind(db) {
            ApiKind::Wasm { .. } => {
                format!("#[wasm_bindgen(setter, js_name = {})]", field.name(db))
            }
            ApiKind::Node => format!("#[napi(setter, js_name = \"{}\")]", field.name(db)),
        }
    }

    /// Returns the error type for this API.
    pub fn result_type(&self, db: &dyn Db, ok_type: &str) -> String {
        match self.kind(db) {
            ApiKind::Wasm { .. } => format!("Result<{}, JsValue>", ok_type),
            ApiKind::Node => format!("napi::Result<{}>", ok_type),
        }
    }

    /// Returns an error expression with the given reason.
    pub fn error(&self, db: &dyn Db, reason: &str) -> String {
        match self.kind(db) {
            ApiKind::Wasm { .. } => format!("JsValue::from_str(&{reason})"),
            ApiKind::Node => format!("napi::Error::from_reason({reason})"),
        }
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
    pub fn manifest_deps(&self, db: &dyn Db) -> Option<String> {
        match self.kind(db) {
            ApiKind::Wasm { .. } => Some(
                indoc::indoc! {r#"
                    wasm-bindgen = "0.2"
                "#}
                .to_string(),
            ),
            ApiKind::Node => Some(
                indoc::indoc! {r#"
                    napi = { version = "3", features = ["napi10"] }
                    napi-derive = { version = "3" }
                "#}
                .to_string(),
            ),
        }
    }

    /// Returns the Cargo manifest build dependencies required by this API.
    pub fn manifest_build_deps(&self, db: &dyn Db) -> Option<String> {
        match self.kind(db) {
            ApiKind::Wasm { .. } => None,
            ApiKind::Node => Some(
                indoc::indoc! {r#"

                    [build-dependencies]
                    napi-build = "2""#}
                .to_string(),
            ),
        }
    }

    /// Returns JavaScript initialization code needed by this API before exports
    /// are used.
    pub fn js_mod_init(&self, db: &dyn Db, src_path: &str) -> Option<String> {
        match self.kind(db) {
            ApiKind::Wasm {
                target: WasmTarget::Web,
            } => Some(format!("import init from '{}';\nawait init();\n", src_path)),
            ApiKind::Wasm {
                target: WasmTarget::Node,
            }
            | ApiKind::Node => None,
        }
    }
}
