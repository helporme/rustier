//! JavaScript environments and values.

use crate::transport::{JsEnvHandle, JsValueHandle};

pub use crate::transport::JsValueType;

/// A non-owning handle to a JavaScript environment.
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct JsEnv(JsEnvHandle);

impl JsEnv {
    /// Creates a JavaScript number.
    pub fn new_number(&self, value: f64) -> JsValue {
        self.0.new_number(value).into()
    }

    /// Creates a JavaScript string containing `value`.
    pub fn new_string(&self, value: impl AsRef<str>) -> JsValue {
        self.0.new_string(value.as_ref()).into()
    }

    /// Creates a unique JavaScript symbol with an optional description.
    pub fn new_symbol(&self, description: Option<&str>) -> JsValue {
        self.0.new_symbol(description).into()
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

impl From<JsEnvHandle> for JsEnv {
    fn from(handle: JsEnvHandle) -> Self {
        Self(handle)
    }
}

impl From<JsEnv> for JsEnvHandle {
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
    handle: JsValueHandle,
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

    pub fn as_f32(&self) -> Option<f32> {
        self.as_f64().map(|value| value as f32)
    }

    pub fn as_i128(&self) -> Option<i128> {
        self.handle.as_i128()
    }

    pub fn as_u128(&self) -> Option<u128> {
        self.handle.as_u128()
    }

    pub fn as_i64(&self) -> Option<i64> {
        self.handle.as_i64()
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.handle.as_u64()
    }

    pub fn as_usize(&self) -> Option<usize> {
        self.as_u64()?.try_into().ok()
    }

    pub fn as_i16(&self) -> Option<i16> {
        let value = self.as_f64()?;
        (value.is_finite()
            && value.fract() == 0.0
            && value >= i16::MIN as f64
            && value <= i16::MAX as f64)
            .then_some(value as i16)
    }

    pub fn as_i32(&self) -> Option<i32> {
        let value = self.as_f64()?;
        (value.is_finite()
            && value.fract() == 0.0
            && value >= i32::MIN as f64
            && value <= i32::MAX as f64)
            .then_some(value as i32)
    }

    pub fn as_u16(&self) -> Option<u16> {
        let value = self.as_f64()?;
        (value.is_finite()
            && value.fract() == 0.0
            && value >= u16::MIN as f64
            && value <= u16::MAX as f64)
            .then_some(value as u16)
    }

    pub fn as_u32(&self) -> Option<u32> {
        let value = self.as_f64()?;
        (value.is_finite()
            && value.fract() == 0.0
            && value >= u32::MIN as f64
            && value <= u32::MAX as f64)
            .then_some(value as u32)
    }

    /// The boolean, or `None` if this is not a JavaScript boolean.
    pub fn as_bool(&self) -> Option<bool> {
        self.handle.as_bool()
    }

    /// Copies the string into a Rust [`String`], or returns `None` if this is
    /// not a JavaScript string.
    ///
    /// Note that unpaired UTF-16 surrogates are replaced with U+FFFD.
    pub fn to_string(&self) -> Option<String> {
        self.handle.to_string()
    }
}

impl From<JsValueHandle> for JsValue {
    fn from(handle: JsValueHandle) -> Self {
        Self { handle }
    }
}

impl From<JsValue> for JsValueHandle {
    fn from(value: JsValue) -> Self {
        value.handle
    }
}
