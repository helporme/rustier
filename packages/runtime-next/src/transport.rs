#[cfg(not(target_family = "wasm"))]
pub mod node;
#[cfg(target_family = "wasm")]
pub mod wasm;

#[cfg(not(target_family = "wasm"))]
pub use node::{EnvHandle as JsEnvHandle, ValueHandle as JsValueHandle};
#[cfg(target_family = "wasm")]
pub use wasm::{EnvHandle as JsEnvHandle, ValueHandle as JsValueHandle};

pub const WASM_RUNTIME_FILENAME: &str = "rustier-wasm-rt.mjs";
pub const WASM_RUNTIME: &str = include_str!("transport/rustier-wasm-rt.mjs");

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum JsValueType {
    String = 0,
    Number = 1,
    Bigint = 2,
    Boolean = 3,
    Symbol = 4,
    Undefined = 5,
    Null = 6,
    Object = 7,
    Function = 8,
}
