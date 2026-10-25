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

    /// Compares values using JavaScript's strict equality operator (`===`).
    ///
    /// Values from different environments are unequal.
    pub fn strict_eq(&self, other: &JsValue) -> bool {
        self.handle.strict_eq(&other.handle)
    }

    /// Compares values using JavaScript's `Object.is` semantics.
    ///
    /// Unlike [`Self::strict_eq`], this considers `NaN` equal to itself and
    /// distinguishes positive and negative zero.
    pub fn soft_eq(&self, other: &JsValue) -> bool {
        self.handle.soft_eq(&other.handle)
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
    pub fn get(&self, key: impl Into<Key>) -> Result<JsValue, JsValue> {
        let key = key.into().into_handle(self.env());

        self.handle.get(&key).map(Into::into).map_err(Into::into)
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
    pub fn set(&self, key: impl Into<Key>, value: &JsValue) -> Result<bool, JsValue> {
        let key = key.into().into_handle(self.env());

        self.handle.set(&key, &value.handle).map_err(Into::into)
    }

    /// Calls this function with JavaScript `undefined` as its receiver.
    ///
    /// # Errors
    ///
    /// Returns the thrown value if the call raises a JavaScript exception.
    /// A non-callable value or an argument from another environment produces a
    /// `TypeError`.
    pub fn apply(&self, args: &[JsValue]) -> Result<JsValue, JsValue> {
        self.handle
            .apply(args.iter().map(|value| &value.handle))
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Calls this function with `this` as its receiver.
    ///
    /// # Errors
    ///
    /// Returns the thrown value if the call raises a JavaScript exception.
    /// A non-callable value, or a receiver or argument from another environment,
    /// produces a `TypeError`.
    pub fn apply_as(&self, this: &JsValue, args: &[JsValue]) -> Result<JsValue, JsValue> {
        self.handle
            .apply_as(&this.handle, args.iter().map(|value| &value.handle))
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Calls the method named `method`, using this value as its receiver.
    ///
    /// # Errors
    ///
    /// Returns the thrown value if property lookup or the call raises a JavaScript
    /// exception. A non-object target, a non-callable method, or an argument from
    /// another environment produces a `TypeError`.
    pub fn call(&self, method: impl Into<Key>, args: &[JsValue]) -> Result<JsValue, JsValue> {
        let method = method.into().into_handle(self.env());

        self.handle
            .call(&method, args.iter().map(|value| &value.handle))
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Returns whether this object or its prototype chain has `key`.
    ///
    /// Returns the thrown value if lookup fails.
    pub fn has(&self, key: impl Into<Key>) -> Result<bool, JsValue> {
        let key = key.into().into_handle(self.env());

        self.handle.has(&key).map_err(Into::into)
    }

    /// Deletes an own property, returning `false` if deletion is rejected.
    ///
    /// Returns the thrown value if deletion fails.
    pub fn delete(&self, key: impl Into<Key>) -> Result<bool, JsValue> {
        let key = key.into().into_handle(self.env());

        self.handle.delete(&key).map_err(Into::into)
    }

    /// Invokes this constructor with `args`.
    ///
    /// Returns the thrown value if construction fails.
    pub fn construct(&self, args: &[JsValue]) -> Result<JsValue, JsValue> {
        self.handle
            .construct(args.iter().map(|value| &value.handle))
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Returns all own string and symbol keys, including non-enumerable keys.
    ///
    /// Keys are returned as [`Key::Value`] to preserve JavaScript strings exactly.
    /// Returns the thrown value if enumeration fails.
    pub fn own_keys(&self) -> Result<Vec<Key>, JsValue> {
        self.handle
            .own_keys()
            .map(|keys| keys.into_iter().map(|key| Key::Value(key.into())).collect())
            .map_err(Into::into)
    }

    /// Returns an own property's descriptor, or `undefined` if it does not exist.
    ///
    /// Returns the thrown value if lookup fails.
    pub fn get_own_property_descriptor(&self, key: impl Into<Key>) -> Result<JsValue, JsValue> {
        let key = key.into().into_handle(self.env());

        self.handle
            .get_own_property_descriptor(&key)
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Defines an own property, returning `false` if the definition is rejected.
    ///
    /// # Errors
    ///
    /// `descriptor` is a JavaScript property descriptor object.
    /// Returns the thrown value if the descriptor or operation is invalid.
    pub fn define_property(
        &self,
        key: impl Into<Key>,
        descriptor: &JsValue,
    ) -> Result<bool, JsValue> {
        let key = key.into().into_handle(self.env());

        self.handle
            .define_property(&key, &descriptor.handle)
            .map_err(Into::into)
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

/// A JavaScript property key.
///
/// Numbers and JavaScript values undergo JavaScript's property-key conversion
/// when used. Symbols retain their identity.
#[derive(Debug, Clone)]
pub enum Key {
    /// A JavaScript value to use as a key.
    Value(JsValue),
    /// A string key.
    String(String),
    /// A numeric key.
    Number(f64),
}

impl Key {
    fn into_handle(self, env: JsEnv) -> JsValueHandle {
        match self {
            Self::Value(value) => value.handle,
            Self::String(value) => env.0.new_string(&value),
            Self::Number(value) => env.0.new_number(value),
        }
    }
}

impl From<JsValue> for Key {
    fn from(value: JsValue) -> Self {
        Self::Value(value)
    }
}

impl From<&JsValue> for Key {
    fn from(value: &JsValue) -> Self {
        Self::Value(value.clone())
    }
}

impl From<String> for Key {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for Key {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<&String> for Key {
    fn from(value: &String) -> Self {
        Self::String(value.clone())
    }
}

macro_rules! number_keys {
    ($($type:ty),* $(,)?) => {
        $(
            impl From<$type> for Key {
                fn from(value: $type) -> Self {
                    Self::Number(value as f64)
                }
            }
        )*
    };
}

number_keys!(f64, f32, u8, u16, u32, usize, i8, i16, i32, isize);
