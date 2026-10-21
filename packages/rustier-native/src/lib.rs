#![recursion_limit = "256"]

/// JavaScript environments and values.
///
/// [`JsEnv`](js::JsEnv) is supplied by Rustier to exported Rust functions.
/// [`JsValue`](js::JsValue) keeps a JavaScript value alive without copying it.
///
/// These handles may only be used on their owning JavaScript thread.
pub mod js {
    pub use rustier_ext::runtime::js::{JsEnv, JsValue, JsValueType, Key};
}

pub mod compiler;
#[cfg(feature = "napi-binding")]
pub mod server;
