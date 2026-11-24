use crate::{
    codegen::fragment::{FragmentOnce, fragment},
    db::Db,
    ir::transport::{TransportFunction, TransportTypeRs},
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

impl TransportTypeRs {
    pub(super) fn rust_name(self) -> &'static str {
        match self {
            Self::String => "::std::string::String",
            Self::Bool => "bool",
            Self::F64 => "f64",
            Self::Usize => "usize",
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::I128 => "i128",
            Self::U128 => "u128",
            Self::Void => "()",
            Self::JsValueHandle => "::rustier_runtime::transport::JsValueHandle",
        }
    }
}

pub(super) fn render_transport_preamble_js(
    backend: &TransportBackend,
    bin_file_path: &String,
    imports: &[&str],
) -> impl FragmentOnce {
    fragment(move |p| match backend {
        TransportBackend::Node(backend) => {
            node::render_transport_preamble_js(backend, bin_file_path).apply(p)
        }
        TransportBackend::Wasm(backend) => {
            wasm::render_transport_preamble_js(backend, bin_file_path, imports).apply(p)
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
    bind_imports: Option<TransportFunction<'db>>,
    functions: impl IntoIterator<Item = TransportFunction<'db>>,
) -> String {
    match backend {
        TransportBackend::Node(_) => node::render_registration(db, bind_imports, functions),
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
