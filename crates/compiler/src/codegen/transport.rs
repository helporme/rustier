use crate::{
    codegen::fragment::{FragmentOnce, fragment},
    db::Db,
    ir::transport::TransportFunction,
};

mod node;
mod wasm;

pub use node::TransportBackendNode;
pub use wasm::{TransportBackendWasm, WasmImport};

/// Backend-specific transport code generation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TransportBackend {
    Node(TransportBackendNode),
    Wasm(TransportBackendWasm),
}

pub(super) fn render_transport_preamble_js(
    backend: &TransportBackend,
    bin_file_path: &String,
) -> impl FragmentOnce {
    fragment(move |p| match backend {
        TransportBackend::Node(backend) => {
            node::render_transport_preamble_js(backend, bin_file_path).apply(p)
        }
        TransportBackend::Wasm(backend) => {
            wasm::render_transport_preamble_js(backend, bin_file_path).apply(p)
        }
    })
}

pub(super) fn render_transport_preamble_rs(backend: &TransportBackend) -> impl FragmentOnce {
    fragment(move |p| match backend {
        TransportBackend::Node(_) => node::render_transport_preamble_rs().apply(p),
        TransportBackend::Wasm(backend) => wasm::render_transport_preamble_rs(backend).apply(p),
    })
}

pub(super) fn render_transport_registration_rs<'db>(
    backend: &TransportBackend,
    db: &'db dyn Db,
    functions: impl IntoIterator<Item = TransportFunction<'db>>,
) -> String {
    match backend {
        TransportBackend::Node(_) => node::render_registration(db, functions),
        TransportBackend::Wasm(_) => String::new(),
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
            TransportBackend::Node(backend) => {
                backend.render_rs_adapter_with_body(self, db, render_body)
            }
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
            TransportBackend::Node(backend) => {
                backend.render_js_ffi_call_stmt(self, db, arguments, result_var)
            }
            TransportBackend::Wasm(backend) => {
                backend.render_js_ffi_call_stmt(self, db, arguments, result_var)
            }
        }
    }
}
