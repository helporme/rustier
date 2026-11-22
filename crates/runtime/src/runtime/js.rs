//! JavaScript environments and values.

use crate::transport::{JsEnvHandle, JsValueHandle};

pub use crate::transport::JsValueType;

/// A non-owning handle to a JavaScript environment.
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct JsEnv(JsEnvHandle);

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
