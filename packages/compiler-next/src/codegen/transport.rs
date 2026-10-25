use crate::{
    codegen::fragment::{FragmentOnce, FragmentPrint, frag},
    db::Db,
    ir::transport::TransportFunction,
};

mod wasm;

pub use wasm::{TransportBackendWasm, WasmImport};

/// Module IDs used to generate transport adapters.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct TransportCodegenOpts {
    /// The shared JS backend module imported by generated snippet wrappers.
    pub ffi_module_id: String,
    /// The backend binary module ID, such as a `.wasm` or `.node` file path.
    pub ffi_bin_id: String,
}

/// Backend-specific transport code generation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TransportBackend {
    Wasm(TransportBackendWasm),
}

impl TransportBackend {
    /// Shared backend initialization and bindings used by per-source FFI modules.
    pub fn render_js_backend(&self, opts: &TransportCodegenOpts) -> String {
        let preamble = render_transport_preamble_js(self, opts);
        frag!(
            {preamble}@
            export {{ __rustier_exports, __rustier_values }};@
        )
        .print()
    }
}

pub(super) fn render_transport_preamble_rs(backend: &TransportBackend) -> impl FragmentOnce {
    match backend {
        TransportBackend::Wasm(backend) => wasm::render_transport_preamble_rs(backend),
    }
}

pub(super) fn render_transport_preamble_js(
    backend: &TransportBackend,
    opts: &TransportCodegenOpts,
) -> String {
    match backend {
        TransportBackend::Wasm(backend) => {
            wasm::render_transport_preamble_js(backend, opts).print()
        }
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
