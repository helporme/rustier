use salsa::Setter;

use crate::codegen::{db::Db, ir};

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

    /// Returns the function header for this API.
    pub fn fn_header(&self, db: &dyn Db, _fn: ir::Fn) -> String {
        let fn_name = _fn.mangled_name(db);
        match self.kind(db) {
            ApiKind::Wasm { .. } => format!("#[wasm_bindgen(js_name = {})]", fn_name),
            ApiKind::Node => format!("#[napi(js_name = \"{}\")]", fn_name),
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
