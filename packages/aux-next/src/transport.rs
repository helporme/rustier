#[cfg(not(target_family = "wasm"))]
mod node;
#[cfg(not(target_family = "wasm"))]
pub use napi_sys;
#[cfg(target_family = "wasm")]
pub mod wasm;

#[cfg(not(target_family = "wasm"))]
pub use node::{JsEnvHandle, JsValueHandle};
#[cfg(target_family = "wasm")]
pub use wasm::{JsEnvHandle, JsValueHandle};

#[derive(Debug)]
pub enum TransportError {
    Static(&'static str),
    Dynamic(String),
}

impl AsRef<str> for TransportError {
    fn as_ref(&self) -> &str {
        match self {
            Self::Static(message) => message,
            Self::Dynamic(message) => message,
        }
    }
}

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
