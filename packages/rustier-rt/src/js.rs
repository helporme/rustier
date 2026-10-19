//! JavaScript environments and values.

#[cfg(not(target_family = "wasm"))]
#[path = "js/backend/node.rs"]
pub mod backend;
#[cfg(target_family = "wasm")]
#[path = "js/backend/wasm.rs"]
pub mod backend;

pub mod wasm;

/// The type of a JavaScript value.
///
/// This follows JavaScript's `typeof` operator, except that `null` has its own
/// variant.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum JsValueType {
    /// A primitive string.
    String = 0,
    /// A number.
    Number = 1,
    /// A bigint.
    Bigint = 2,
    /// A boolean.
    Boolean = 3,
    /// A symbol.
    Symbol = 4,
    /// The `undefined` value.
    Undefined = 5,
    /// The `null` value.
    Null = 6,
    /// An object other than a function.
    Object = 7,
    /// A function.
    Function = 8,
}

/// A non-owning handle to a JavaScript environment.
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct JsEnv(backend::EnvHandle);

impl JsEnv {
    /// Creates an environment handle from its raw representation.
    ///
    /// # Safety
    ///
    /// On Node.js, `raw` must be a live Node-API environment pointer on the
    /// current JavaScript thread. On WebAssembly, it must be zero.
    pub unsafe fn from_raw(raw: usize) -> Self {
        Self(unsafe { backend::EnvHandle::from_raw(raw) })
    }

    /// Returns the raw environment handle.
    ///
    /// This does not release or extend the lifetime of the environment.
    ///
    /// # Safety
    ///
    /// The returned handle must not be used after the environment is destroyed
    /// or from another thread.
    pub unsafe fn into_raw(self) -> usize {
        self.0.into_raw()
    }

    /// Creates a JavaScript number.
    pub fn new_number(&self, value: f64) -> JsValue {
        self.0.new_number(value).into()
    }

    /// Creates a JavaScript string containing `value`.
    pub fn new_string(&self, value: &str) -> JsValue {
        self.0.new_string(value).into()
    }

    /// Creates a JavaScript boolean.
    pub fn new_boolean(&self, value: bool) -> JsValue {
        self.0.new_boolean(value).into()
    }

    /// Returns the JavaScript `null` value.
    pub fn null(&self) -> JsValue {
        self.0.null().into()
    }

    /// Returns the JavaScript `undefined` value.
    pub fn undefined(&self) -> JsValue {
        self.0.undefined().into()
    }

    /// Creates an empty JavaScript object.
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

/// An owned handle to a JavaScript value.
///
/// The value behind the handle may be garbage-collected once no handles or
/// other JavaScript references keep it alive.
#[derive(Debug, Clone)]
pub struct JsValue {
    handle: backend::ValueHandle,
}

impl JsValue {
    /// The environment this value belongs to.
    pub fn env(&self) -> JsEnv {
        self.handle.env().into()
    }

    /// The type of this value without converting it.
    pub fn type_of(&self) -> JsValueType {
        self.handle.type_of()
    }

    /// The number, or `None` if this is not a JavaScript number.
    pub fn as_f64(&self) -> Option<f64> {
        self.handle.as_f64()
    }

    /// The boolean, or `None` if this is not a JavaScript boolean.
    pub fn as_bool(&self) -> Option<bool> {
        self.handle.as_bool()
    }

    /// Copies the string into a Rust [`String`], or returns `None` if this is
    /// not a JavaScript string.
    ///
    /// Note that unpaired UTF-16 surrogates are replaced with U+FFFD.
    pub fn as_string(&self) -> Option<String> {
        self.handle.as_string()
    }

    /// The value of the property named `key`.
    ///
    /// Searches the prototype chain and returns JavaScript `undefined` if the
    /// property does not exist.
    ///
    /// # Errors
    ///
    /// Returns the thrown value if property access raises a JavaScript exception.
    /// A target that is neither an object nor a function produces a `TypeError`.
    pub fn get(&self, key: &str) -> Result<JsValue, JsValue> {
        self.handle.get(key).map(Into::into).map_err(Into::into)
    }

    /// Assigns `value` to the property named `key`.
    ///
    /// Returns `true` if the write is accepted, or `false` if it is rejected,
    /// for example by a non-writable property.
    ///
    /// # Errors
    ///
    /// Returns the thrown value if assignment raises a JavaScript exception.
    /// A target that is neither an object nor a function, or a value from another
    /// environment, produces a `TypeError`.
    pub fn set(&self, key: &str, value: &JsValue) -> Result<bool, JsValue> {
        self.handle.set(key, &value.handle).map_err(Into::into)
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
