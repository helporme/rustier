//! IR describing rust/js boundary api, implemented by specific backend (wasm/napi).

use super::runtime::{BigIntType, RsSignature, RsType};
use crate::db::Db;

/// A transport function defined to be called from js.
#[salsa::interned]
pub struct TransportFunction<'db> {
    #[returns(ref)]
    pub symbol: String,
    #[returns(ref)]
    pub inputs: Vec<TransportTypeRs>,
    pub ret_type: TransportTypeRs,
    pub can_return_error: bool,
}

impl<'db> TransportFunction<'db> {
    /// Lowers a resolved runtime signature to the types carried across the bridge.
    pub fn lower(db: &'db dyn Db, symbol: &str, signature: RsSignature<'_>) -> Self {
        Self::new(
            db,
            symbol,
            signature
                .inputs
                .iter()
                .copied()
                .map(TransportTypeRs::from)
                .collect::<Vec<_>>(),
            TransportTypeRs::from(signature.ret_type),
            signature.can_return_error,
        )
    }

    /// The JavaScript finalizer releases a Rust handle by its pointer.
    pub fn handle_drop(db: &'db dyn Db) -> Self {
        Self::new(
            db,
            "__rustier_drop_handle",
            vec![TransportTypeRs::Usize],
            TransportTypeRs::Void,
            false,
        )
    }
}

/// The Rust-side representation of a value carried by transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TransportTypeRs {
    String,
    Bool,
    F64,
    Usize,
    I64,
    U64,
    I128,
    U128,
    Void,
    JsValueHandle,
}

impl From<RsType> for TransportTypeRs {
    fn from(ty: RsType) -> Self {
        match ty {
            RsType::String => Self::String,
            RsType::Bool => Self::Bool,
            RsType::Void => Self::Void,
            RsType::JsValue => Self::JsValueHandle,
            RsType::RustHandle { .. } | RsType::BigInt(BigIntType::Usize) => Self::Usize,
            RsType::BigInt(BigIntType::I64) => Self::I64,
            RsType::BigInt(BigIntType::U64) => Self::U64,
            RsType::BigInt(BigIntType::I128) => Self::I128,
            RsType::BigInt(BigIntType::U128) => Self::U128,
            RsType::I8
            | RsType::I16
            | RsType::I32
            | RsType::U8
            | RsType::U16
            | RsType::U32
            | RsType::F32
            | RsType::F64 => Self::F64,
        }
    }
}

pub type DynError = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type Result2<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub fn test() -> Result2<u32> {
    other()?;
    Ok(0)
}

pub fn other() -> Result<(), String> {
    Ok(())
}

/// The JS-side representation of a value carried by transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TransportTypeJs {
    String,
    Boolean,
    Number,
    BigInt(BigIntType),
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
            Self::Usize => TransportTypeJs::BigInt(BigIntType::Usize),
            Self::I64 => TransportTypeJs::BigInt(BigIntType::I64),
            Self::U64 => TransportTypeJs::BigInt(BigIntType::U64),
            Self::I128 => TransportTypeJs::BigInt(BigIntType::I128),
            Self::U128 => TransportTypeJs::BigInt(BigIntType::U128),
            Self::Void => TransportTypeJs::Undefined,
            Self::JsValueHandle => TransportTypeJs::Any,
        }
    }
}
