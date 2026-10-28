//! IR defining a runtime behavior of rustier, like supported types or kind of macros.

use std::collections::HashSet;

mod analysis;

pub use analysis::RuntimeAnalysis;

use crate::{
    db::Db,
    ir::{
        js::{JsPart, JsProgram, RsMacroSrc},
        transport::{TransportFunction, TransportTypeRs},
    },
};

impl JsProgram {
    /// The runtime associated with this program.
    pub fn runtime<'db>(self, db: &'db dyn Db) -> Runtime<'db> {
        Runtime::new(db, self)
    }
}

/// Runtime behavior collected from a JavaScript program.
#[salsa::interned]
pub struct Runtime<'db> {
    js_program: JsProgram,
}

impl<'db> Runtime<'db> {
    /// Macros encountered in the program.
    pub fn macros(self, db: &'db dyn Db) -> Vec<RsMacro<'db>> {
        let mut seen = HashSet::new();
        let mut macros = Vec::new();

        for module in self.js_program(db).modules(db) {
            for src in module.macros(db) {
                let macro_ = src.as_macro(db);
                if seen.insert(macro_) {
                    macros.push(macro_);
                }
            }
        }

        macros.sort_by(|left, right| left.symbol(db).cmp(right.symbol(db)));
        macros
    }
}

/// The `rs` macro.
#[salsa::interned]
pub struct RsMacro<'db> {
    #[returns(ref)]
    pub body: Vec<RsPart>,
}

#[derive(Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum RsPart {
    /// Rust source text preserved verbatim.
    Text(String),
    /// The next positional input expression.
    Input,
}

#[salsa::tracked]
impl<'db> RsMacroSrc<'db> {
    /// Converts this macro source into an [`RsMacro`].
    #[salsa::tracked]
    pub fn as_macro(self, db: &'db dyn Db) -> RsMacro<'db> {
        let body: Vec<_> = self
            .body(db)
            .iter()
            .map(|part| match part {
                JsPart::Text(range) => {
                    let source = range.file(db).contents(db);
                    RsPart::Text(
                        source[range.start(db) as usize..range.end(db) as usize].to_owned(),
                    )
                }
                JsPart::Interpolation { .. } => RsPart::Input,
            })
            .collect();

        RsMacro::new(db, body)
    }
}

#[salsa::tracked]
impl<'db> RsMacro<'db> {
    /// A stable macro export name.
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        let mut hash = blake3::Hasher::new();
        for part in self.body(db) {
            match part {
                RsPart::Text(text) => {
                    hash.update(&[0]);
                    hash.update(&(text.len() as u64).to_le_bytes());
                    hash.update(text.as_bytes());
                }
                RsPart::Input => {
                    hash.update(&[1]);
                }
            }
        }
        format!("__rustier_{}", hash.finalize().to_hex())
    }

    /// Rust-side input types in interpolation order.
    pub fn inputs(self, db: &'db dyn Db) -> &'db Vec<RsType> {
        self.infer_input_types(db)
    }

    /// Rust-side type returned by this macro.
    pub fn ret_type(self, db: &'db dyn Db) -> RsType {
        self.infer_ret_type(db)
    }

    /// Transport-level function of this macro.
    #[salsa::tracked]
    pub fn transport(self, db: &'db dyn Db) -> TransportFunction<'db> {
        TransportFunction::new(
            db,
            self.symbol(db),
            self.inputs(db)
                .iter()
                .map(|type_| type_.transport())
                .collect::<Vec<_>>(),
            self.ret_type(db).transport(),
        )
    }
}

/// A Rust-side runtime type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum RsType {
    I8,
    I16,
    I32,
    U8,
    U16,
    U32,
    F32,
    F64,
    /// Rust unit (`()`).
    Void,
    /// Matches `aux_next::runtime::js::JsValue`.
    JsValue,
}

/// A js-side runtime type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum JsType {
    Number,
    /// JavaScript `undefined`.
    Undefined,
    /// An arbitrary JS value.
    Any,
}

impl RsType {
    /// Chooses the Rust transport representation needed to carry this runtime type.
    pub fn transport(self) -> TransportTypeRs {
        match self {
            Self::Void => TransportTypeRs::Void,
            Self::JsValue => TransportTypeRs::JsValueHandle,
            Self::I8
            | Self::I16
            | Self::I32
            | Self::U8
            | Self::U16
            | Self::U32
            | Self::F32
            | Self::F64 => TransportTypeRs::F64,
        }
    }

    /// Maps this runtime type to its JS-facing representation.
    pub fn js(self) -> JsType {
        match self {
            Self::Void => JsType::Undefined,
            Self::JsValue => JsType::Any,
            Self::I8
            | Self::I16
            | Self::I32
            | Self::U8
            | Self::U16
            | Self::U32
            | Self::F32
            | Self::F64 => JsType::Number,
        }
    }
}
