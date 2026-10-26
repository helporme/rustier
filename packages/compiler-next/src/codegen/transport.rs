use crate::{codegen::fragment::FragmentOnce, db::Db, ir::transport::TransportFunction};

mod wasm;

pub use wasm::{TransportBackendWasm, WasmImport};

/// Backend-specific transport code generation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TransportBackend {
    Wasm(TransportBackendWasm),
}

pub(super) fn render_transport_preamble_js(
    backend: &TransportBackend,
    bin_file_path: &String,
) -> impl FragmentOnce {
    match backend {
        TransportBackend::Wasm(backend) => {
            wasm::render_transport_preamble_js(backend, bin_file_path)
        }
    }
}

pub(super) fn render_transport_preamble_rs(backend: &TransportBackend) -> impl FragmentOnce {
    match backend {
        TransportBackend::Wasm(backend) => wasm::render_transport_preamble_rs(backend),
    }
}

impl<'db> TransportFunction<'db> {
    pub(super) fn render_rs_adapter_with_body(
        self,
        db: &'db dyn Db,
        backend: &TransportBackend,
        render_body: impl FnOnce(&str, &[String]) -> String,
    ) -> String {
        match backend {
            TransportBackend::Wasm(backend) => {
                backend.render_rs_adapter_with_body(self, db, render_body)
            }
        }
    }

    pub(super) fn render_js_ffi_call_stmt(
        self,
        db: &'db dyn Db,
        backend: &TransportBackend,
        arguments: &[String],
        result_var: &str,
    ) -> String {
        match backend {
            TransportBackend::Wasm(backend) => {
                backend.render_js_ffi_call_stmt(self, db, arguments, result_var)
            }
        }
    }
}
