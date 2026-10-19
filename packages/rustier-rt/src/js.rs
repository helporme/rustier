//! Rust-facing handles to JavaScript environments and values.
//!
//! Rustier supplies [`JsEnv`] to exported Rust functions without adding an
//! environment argument to their JavaScript API. [`JsValue`] owns a strong root
//! to a JavaScript value, not a serialized copy. Cloning retains the same value;
//! dropping all roots allows collection if the value is otherwise unreachable.
//!
//! Handles are neither [`Send`] nor [`Sync`]. They must be used and dropped on
//! their owning JavaScript thread while the environment is alive.
//!
//! [`backend`] exposes low-level handles and raw ownership transfers for custom
//! storage. [`wasm`] contains the JavaScript host runtime used by Wasm bridges.
//! For handles to Rust allocations referenced by JS, see [`crate::rust_handles`].

#[cfg(not(target_family = "wasm"))]
#[path = "js/backend/node.rs"]
pub mod backend;
#[cfg(target_family = "wasm")]
#[path = "js/backend/wasm.rs"]
pub mod backend;

pub mod wasm;

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

/// A particular JavaScript environment, for example a web worker.
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct JsEnv(backend::EnvHandle);

impl JsEnv {
    /// Creates a new `JsEnv` from a raw JavaScript environment pointer.
    ///
    /// # Safety
    ///
    /// The `raw` value must be a valid JavaScript environment pointer.
    pub unsafe fn from_raw(raw: usize) -> Self {
        Self(unsafe { backend::EnvHandle::from_raw(raw) })
    }

    pub unsafe fn into_raw(self) -> usize {
        self.0.into_raw()
    }

    pub fn new_number(&self, value: f64) -> JsValue {
        self.0.new_number(value).into()
    }

    pub fn new_string(&self, value: &str) -> JsValue {
        self.0.new_string(value).into()
    }

    pub fn new_boolean(&self, value: bool) -> JsValue {
        self.0.new_boolean(value).into()
    }

    pub fn null(&self) -> JsValue {
        self.0.null().into()
    }

    pub fn undefined(&self) -> JsValue {
        self.0.undefined().into()
    }

    pub fn new_object(&self) -> JsValue {
        self.0.new_object().into()
    }
}

impl From<backend::EnvHandle> for JsEnv {
    fn from(handle: backend::EnvHandle) -> Self {
        Self(handle)
    }
}

impl From<JsEnv> for backend::EnvHandle {
    fn from(js: JsEnv) -> Self {
        js.0
    }
}

#[derive(Debug, Clone)]
pub struct JsValue {
    handle: backend::ValueHandle,
}

impl JsValue {
    pub fn env(&self) -> JsEnv {
        self.handle.env().into()
    }

    pub fn type_of(&self) -> JsValueType {
        self.handle.type_of()
    }

    pub fn as_f64(&self) -> Option<f64> {
        self.handle.as_f64()
    }

    pub fn as_bool(&self) -> Option<bool> {
        self.handle.as_bool()
    }

    pub fn as_string(&self) -> Option<String> {
        self.handle.as_string()
    }
}

impl From<backend::ValueHandle> for JsValue {
    fn from(handle: backend::ValueHandle) -> Self {
        Self { handle }
    }
}

impl From<JsValue> for backend::ValueHandle {
    fn from(value: JsValue) -> Self {
        value.handle
    }
}
