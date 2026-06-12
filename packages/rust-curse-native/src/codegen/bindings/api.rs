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
    pub fn fn_header(&self) -> &'static str {
        match self {
            Api::Wasm => "#[wasm_bindgen]",
            Api::Node => "#[napi]",
        }
    }

    /// Returns the error type for this API.
    pub fn result_type(&self, ok_type: &str) -> String {
        match self {
            Api::Wasm => format!("Result<{}, JsValue>", ok_type),
            Api::Node => format!("napi::Result<{}>", ok_type),
        }
    }
}
