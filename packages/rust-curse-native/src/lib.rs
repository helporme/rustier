use napi_derive::napi;

#[napi]
pub fn greet() -> &'static str {
    "Hello from rust!"
}
