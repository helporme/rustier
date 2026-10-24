use crate::{db::Db, ir::RustType};

#[salsa::interned]
pub struct Function<'db> {
    #[returns(ref)]
    pub name: String,
    pub return_type: Type<'db>,
    #[returns(ref)]
    pub body: Vec<Part<'db>>,
}

#[salsa::interned]
pub struct Module<'db> {
    #[returns(ref)]
    pub functions: Vec<Function<'db>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Backend {
    Wasm,
    Node,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Part<'db> {
    Text(String),
    Interpolation { type_: Type<'db> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Type<'db> {
    I16,
    I32,
    U16,
    U32,
    F32,
    F64,
    String,
    JsValue,
    Owned(RustType<'db>),
}

impl<'db> Type<'db> {
    pub fn source(self, db: &'db dyn Db) -> &'db str {
        match self {
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::String => "::std::string::String",
            Self::JsValue => "::runtime_next::runtime::js::JsValue",
            Self::Owned(type_) => type_.source(db),
        }
    }

    pub fn js_conversion(self) -> Option<&'static str> {
        match self {
            Self::I16 => Some("as_i16"),
            Self::I32 => Some("as_i32"),
            Self::U16 => Some("as_u16"),
            Self::U32 => Some("as_u32"),
            Self::F32 => Some("as_f32"),
            Self::F64 => Some("as_f64"),
            Self::String => Some("to_string"),
            Self::JsValue | Self::Owned(_) => None,
        }
    }
}
