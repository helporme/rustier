#[macro_use]
#[path = "ir/common.rs"]
mod common;

#[path = "ir/from_rustdoc.rs"]
mod from_rustdoc;
#[path = "ir/napi.rs"]
mod napi;
#[path = "ir/negative.rs"]
mod negative;
#[path = "ir/ts.rs"]
mod ts;
#[path = "ir/wasm.rs"]
mod wasm;
#[path = "ir/wasm_js.rs"]
mod wasm_js;
