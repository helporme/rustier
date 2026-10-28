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
    String,
    Bool,
    F64,
    Void,
    JsValueHandle,
}

/// The JS-side representation of a value carried by transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TransportTypeJs {
    String,
    Boolean,
    Number,
    Undefined,
    Any,
}

impl TransportTypeRs {
    /// Maps a Rust transport type to its corresponding JS representation.
    pub fn js(self) -> TransportTypeJs {
        match self {
            Self::String => TransportTypeJs::String,
            Self::Bool => TransportTypeJs::Boolean,
            Self::F64 => TransportTypeJs::Number,
            Self::Void => TransportTypeJs::Undefined,
            Self::JsValueHandle => TransportTypeJs::Any,
        }
    }
}
