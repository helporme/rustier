use crate::codegen::{db::Db, ir};

/// Api used for generating bindings code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Api {
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

impl Api {
    /// Returns the prelude code for this API.
    pub fn prelude(&self) -> &str {
        match self {
            Api::Wasm { .. } => "use wasm_bindgen::prelude::*;\n",
            Api::Node => "use napi_derive::napi;\n",
        }
    }

    /// Returns the function header for this API.
    pub fn fn_header(&self, db: &dyn Db, _fn: ir::Fn) -> String {
        let fn_name = _fn.mangled_name(db);
        match self {
            Api::Wasm { .. } => format!("#[wasm_bindgen(js_name = {})]", fn_name),
            Api::Node => format!("#[napi(js_name = \"{}\")]", fn_name),
        }
    }

    /// Returns the error type for this API.
    pub fn result_type(&self, ok_type: &str) -> String {
        match self {
            Api::Wasm { .. } => format!("Result<{}, JsValue>", ok_type),
            Api::Node => format!("napi::Result<{}>", ok_type),
        }
    }

    /// Returns an error expression with the given reason.
    pub fn error(&self, reason: &str) -> String {
        match self {
            Api::Wasm { .. } => format!("JsValue::from_str(&{reason})"),
            Api::Node => format!("napi::Error::from_reason({reason})"),
        }
    }

    /// Returns the contents of `fn main` for this API's build script, if any.
    pub fn build_script_body(&self) -> Option<&'static str> {
        match self {
            Api::Wasm { .. } => None,
            Api::Node => Some("napi_build::setup();"),
        }
    }

    /// Returns JavaScript initialization code needed by this API before exports
    /// are used.
    pub fn js_mod_init(&self, src_path: &str) -> Option<String> {
        match self {
            Api::Wasm {
                target: WasmTarget::Web,
            } => Some(format!("import init from '{}';\nawait init();\n", src_path)),
            Api::Wasm {
                target: WasmTarget::Node,
            }
            | Api::Node => None,
        }
    }
}
