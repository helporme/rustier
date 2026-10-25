//! IR describing rust/js boundary api, implemented by specific backend (wasm/napi).

/// A transport function defined to be called from js.
#[salsa::interned]
pub struct TransportFunction<'db> {
    #[returns(ref)]
    pub symbol: String,
    #[returns(ref)]
    pub inputs: Vec<TransportTypeRs>,
    pub ret_type: TransportTypeRs,
}

/// The Rust-side representation of a value carried by transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TransportTypeRs {
    /// An `aux_next::transport::JsValueHandle`.
    JsValueHandle,
}

/// The JS-side representation of a value carried by transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TransportTypeJs {
    /// An arbitrary JS value
    Any,
}

impl TransportTypeRs {
    /// Maps a Rust transport type to its corresponding JS representation.
    pub fn js(self) -> TransportTypeJs {
        match self {
            Self::JsValueHandle => TransportTypeJs::Any,
        }
    }
}
