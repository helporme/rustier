//! Code generation for the ABI between the Rust runtime and JavaScript.

pub mod js;
mod rust;

pub use rust::{AbiArg, AbiArgs};

/// Transport carrying ABI calls between JavaScript and the Rust runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum AbiTransport {
    WasmBindgen,
    Napi,
}

impl AbiTransport {
    /// The prelude code for this transport.
    pub fn prelude(self) -> &'static str {
        match self {
            AbiTransport::WasmBindgen => "use wasm_bindgen::prelude::*;\n",
            AbiTransport::Napi => "use napi_derive::napi;\n",
        }
    }

    /// The contents of `fn main` for this transport's build script, if any.
    pub const fn build_script_body(self) -> Option<&'static str> {
        match self {
            AbiTransport::WasmBindgen => None,
            AbiTransport::Napi => Some("napi_build::setup();"),
        }
    }
}
