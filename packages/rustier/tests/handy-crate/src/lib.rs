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
