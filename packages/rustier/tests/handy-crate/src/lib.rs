//! Humble structs and enums used in e2e tests.

use rustier::{JsCallError, JsEnv, JsMacro, JsValue, js};

pub struct Message(pub &'static str);

pub struct Borrowed<'a>(pub &'a str);

#[derive(Clone, Copy)]
pub struct CopyValue(pub u32);

pub enum State {
    Ready(String),
}

pub fn deferred<T: 'static>(value: T) -> impl JsMacro<Output = JsValue> {
    js!(return @{value})
}

pub fn point(env: JsEnv, x: u32, y: u32) -> Result<JsValue, JsCallError> {
    let object = js!({}).at(env)?;
    js!(@{object.clone()}.x = @{x}).at(env)?;
    js!(@{object.clone()}.y = @{y}).at(env)?;
    Ok(object)
}

pub fn js_input_at(env: JsEnv, object: JsValue, index: u8) -> Result<JsValue, JsCallError> {
    js!(return [
        @{i8::MIN}, @{i16::MIN}, @{i32::MIN}, @{u8::MAX}, @{u16::MAX}, @{u32::MAX},
        @{1.25f32}, @{-2.5f64}, @{String::from("héllo")}, @{true}, @{()}, @{object}, @{CopyValue(73)}
    ][@{index}]).at(env)
}

pub fn js_no_inputs() -> impl JsMacro<Output = JsValue> {
    js!(return "no inputs")
}

pub fn js_effect(callback: JsValue) -> impl JsMacro<Output = ()> {
    js!(@{callback}("called"))
}

pub fn js_throw() -> impl JsMacro<Output = ()> {
    js!(throw new Error("handy-crate exception"))
}

#[cfg(feature = "invalid-js")]
pub fn invalid_js(env: JsEnv) -> Result<(), JsCallError> {
    js!(const broken = ;).at(env)
}

#[cfg(feature = "nested-rs")]
mod nested_rs;
#[cfg(feature = "nested-rs")]
pub use nested_rs::nested_rs;
