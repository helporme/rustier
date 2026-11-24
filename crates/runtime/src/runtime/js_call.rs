//! Deferred JavaScript calls and their owned Rust inputs.

use std::{any::Any, error::Error, fmt};

use crate::{
    JsEnv, JsValue,
    runtime::rust_handles::RustHandle,
    transport::{JsEnvHandle, JsValueHandle},
};

/// An error from executing a `js!` call. A thrown JavaScript string or an
/// exception's string `message` is preserved, but not the exception object.
#[derive(Debug)]
pub struct JsCallError(String);

impl JsCallError {
    /// Reports a call made before the native addon has bound its JavaScript imports.
    #[doc(hidden)]
    pub fn unbound() -> Self {
        Self("Rustier imports have not been bound for this environment".into())
    }
}

impl fmt::Display for JsCallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for JsCallError {}

impl From<JsValue> for JsCallError {
    fn from(thrown: JsValue) -> Self {
        let thrown = JsValueHandle::from(thrown);
        Self(
            thrown
                .to_string()
                .or_else(|| thrown.exception_message())
                .unwrap_or_else(|| "JavaScript snippet threw an exception".into()),
        )
    }
}

/// Inference marker used by `js!` to reject direct reference inputs. For a
/// reference both implementations apply, so the marker cannot be inferred.
pub trait OwnedInput<Marker> {}
impl<T> OwnedInput<()> for T {}
impl<T: ?Sized> OwnedInput<&()> for &T {}
impl<T: ?Sized> OwnedInput<&()> for &mut T {}

/// An evaluated interpolation held by a deferred `js!` call until execution.
pub enum Input {
    /// A supported Rust integer or float represented as a JavaScript number.
    Number(f64),
    I64(i64),
    U64(u64),
    I128(i128),
    U128(u128),
    /// A Rust boolean.
    Boolean(bool),
    /// Rust `()`, passed to JavaScript as `undefined`.
    Undefined,
    /// An owned Rust string.
    String(String),
    /// An existing JavaScript value, passed without changing its identity.
    Value(JsValue),
    /// An opaque Rust value whose ownership will pass to a JavaScript `Rs` handle.
    Rust(RustHandle),
}

impl Input {
    pub fn new<T: 'static + OwnedInput<M>, M>(value: T) -> Self {
        let any = &value as &dyn Any;
        macro_rules! numbers {
            ($($ty:ty),*) => { $(if let Some(n) = any.downcast_ref::<$ty>() {
                return Self::Number(f64::from(*n));
            })* };
        }
        numbers!(i8, i16, i32, u8, u16, u32, f32, f64);
        if let Some(n) = any.downcast_ref::<i64>() {
            return Self::I64(*n);
        }
        if let Some(n) = any.downcast_ref::<i128>() {
            return Self::I128(*n);
        }
        if let Some(n) = any.downcast_ref::<usize>() {
            return Self::U64(*n as u64);
        }
        if let Some(n) = any.downcast_ref::<u64>() {
            return Self::U64(*n);
        }
        if let Some(n) = any.downcast_ref::<u128>() {
            return Self::U128(*n);
        }
        if let Some(b) = any.downcast_ref::<bool>() {
            return Self::Boolean(*b);
        }
        if any.is::<()>() {
            return Self::Undefined;
        }

        let mut owned = Some(value);
        let any = &mut owned as &mut dyn Any;
        if let Some(s) = any.downcast_mut::<Option<String>>() {
            return Self::String(s.take().unwrap());
        }
        if let Some(v) = any.downcast_mut::<Option<JsValue>>() {
            return Self::Value(v.take().unwrap());
        }
        Self::Rust(RustHandle::new(owned.unwrap()))
    }

    /// Converts a primitive in `env` or returns an existing JavaScript value.
    /// `Err(handle)` hands an opaque value to the bridge to construct its `Rs`
    /// object; it does not mean JavaScript threw an exception.
    pub fn into_value(self, env: JsEnvHandle) -> Result<JsValue, RustHandle> {
        match self {
            Self::Number(n) => Ok(env.new_number(n).into()),
            Self::I64(n) => Ok(env.new_i64(n).into()),
            Self::U64(n) => Ok(env.new_u64(n).into()),
            Self::I128(n) => Ok(env.new_i128(n).into()),
            Self::U128(n) => Ok(env.new_u128(n).into()),
            Self::Boolean(b) => Ok(env.new_boolean(b).into()),
            Self::Undefined => Ok(env.undefined().into()),
            Self::String(s) => Ok(env.new_string(&s).into()),
            Self::Value(v) => Ok(v),
            Self::Rust(handle) => Err(handle),
        }
    }
}

/// A deferred, single-use JavaScript call.
pub trait JsMacro {
    /// [`JsValue`] for a body with a value-bearing return, otherwise `()`.
    type Output;

    /// Executes the body in `env`.
    fn at(self, env: JsEnv) -> Result<Self::Output, JsCallError>;
}
