use crate::codegen::{db::Db, ir};

/// Api used for generating bindings code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Api {
    /// wasm-bindgen
    Wasm,
    /// napi
    Node,
}

impl Api {
    /// Returns the prelude code for this API.
    pub fn prelude(&self) -> &str {
        match self {
            Api::Wasm => "use wasm_bindgen::prelude::*;\n",
            Api::Node => "use napi_derive::napi;\n",
        }
    }

    /// Returns the function header for this API.
    pub fn fn_header(&self, db: &dyn Db, _fn: ir::Fn) -> String {
        let fn_name = _fn.mangled_name(db);
        match self {
            Api::Wasm => format!("#[wasm_bindgen(js_name = {})]", fn_name),
            Api::Node => format!("#[napi(js_name = \"{}\")]", fn_name),
        }
    }

    /// Returns the error type for this API.
    pub fn result_type(&self, ok_type: &str) -> String {
        match self {
            Api::Wasm => format!("Result<{}, JsValue>", ok_type),
            Api::Node => format!("napi::Result<{}>", ok_type),
        }
    }

    /// Returns the contents of `fn main` for this API's build script, if any.
    pub fn build_script_body(&self) -> Option<&'static str> {
        match self {
            Api::Wasm => None,
            Api::Node => Some("napi_build::setup();"),
        }
    }
}
