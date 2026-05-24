//! Codegen layer: ASG, IR, plus the target printers (ts/wasm/napi).

pub mod asg;
pub mod ir;
pub mod mangle;
pub mod napi;
pub mod napi_js;
pub mod projection;
pub mod source_map;
pub mod ts;
pub mod wasm;
pub mod wasm_js;
