mod analysis;
mod fragment;
pub mod js_transform;
pub mod manifest;
mod runtime;
mod transport;

pub use runtime::runtime_exports;
pub use transport::{TransportBackend, TransportBackendNode, TransportBackendWasm, WasmImport};

const RS_IMPORTS: &str = "#[allow(unused_imports)]\nuse ::rustier::{JsCallError, JsEnv, JsMacro, JsValue, JsValueType, Key};\n#[allow(unused_imports)]\nuse ::rustier_js_proc::__js_in_rs as js;\n";
