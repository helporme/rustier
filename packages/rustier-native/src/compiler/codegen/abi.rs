//! Code generation for the ABI between the Rust runtime and JavaScript.

use crate::compiler::ir::abi::transport::AbiTransport;

pub mod js;
mod rust;

pub use rust::{AbiArg, AbiArgs, AbiFunctionRenderOps, render_js_value_adapter};

impl AbiTransport {
    /// The prelude code for this transport.
    pub fn prelude(self) -> &'static str {
        match self {
            AbiTransport::Wasm(_) => "use wasm_bindgen::prelude::*;\n",
            AbiTransport::Napi => "use napi_derive::napi;\n",
        }
    }

    /// The contents of `fn main` for this transport's build script, if any.
    pub const fn build_script_body(self) -> Option<&'static str> {
        match self {
            AbiTransport::Wasm(_) => None,
            AbiTransport::Napi => Some("napi_build::setup();"),
        }
    }
}
