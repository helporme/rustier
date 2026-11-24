#[cfg(not(target_family = "wasm"))]
mod node;
#[cfg(not(target_family = "wasm"))]
pub use napi_sys;
#[cfg(target_family = "wasm")]
pub mod wasm;

#[cfg(not(target_family = "wasm"))]
pub use node::{JsEnvHandle, JsValueHandle, NodeFunction};
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

impl JsValueHandle {
    pub fn as_u128(&self) -> Option<u128> {
        let (negative, magnitude) = self.bigint_parts()?;
        (!negative).then_some(magnitude)
    }

    pub fn as_i128(&self) -> Option<i128> {
        let (negative, magnitude) = self.bigint_parts()?;
        if negative && magnitude == i128::MIN.unsigned_abs() {
            return Some(i128::MIN);
        }
        let value = i128::try_from(magnitude).ok()?;
        Some(if negative { -value } else { value })
    }
}
