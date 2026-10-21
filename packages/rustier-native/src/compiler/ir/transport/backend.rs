#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum TransportBackend {
    Wasm(WasmTransport),
    Napi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub struct WasmTransport {
    pub memory: WasmMemory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum WasmMemory {
    x32,
    x64,
}
