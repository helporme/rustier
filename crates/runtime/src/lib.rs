#![feature(min_specialization)]

pub mod convert;
pub mod errors;
pub mod js;
pub mod js_call;
pub mod rust_handles;
pub mod transport;

pub use convert::{FromJs, ToJs};
pub use js::{JsEnv, JsValue, RsValueHandle};
pub use js_call::{JsCallError, JsMacro};
pub use rustier_js_proc::{FromJs, ToJs};

#[doc(hidden)]
pub use rustier_js_proc::__expand_js;

/// Creates a deferred JavaScript call. Each `@{expr}` is evaluated once, in
/// order, and moved into the call. Inputs must be `'static`.
///
/// Call `.at(env)` to execute the JavaScript body. For example,
/// `js!(return @{x} + 1).at(env)` returns the JavaScript result or an error.
#[macro_export]
macro_rules! js {
    ($($body:tt)*) => { $crate::__expand_js!($crate $($body)*) };
}
