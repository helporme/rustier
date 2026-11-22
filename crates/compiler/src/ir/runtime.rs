//! IR defining a runtime behavior of rustier, like supported types or kind of macros.

use std::collections::HashSet;

mod analysis;
mod diagnostics;

pub use analysis::CompileError;

use crate::{
    db::Db,
    ir::{
        js::{JsPart, JsProgram, RsMacroSrc},
        rs::{JsMacroSrc, RsProgram},
    },
};

/// Runtime behavior collected from JavaScript and Rust source modules.
#[salsa::interned]
pub struct Runtime<'db> {
    js_program: JsProgram,
    rs_program: RsProgram,
}

#[salsa::tracked]
impl<'db> Runtime<'db> {
    /// Distinct `rs` macros from JavaScript modules, ordered by generated symbol.
    #[salsa::tracked(returns(ref))]
    pub fn rs_macros(self, db: &'db dyn Db) -> Vec<RsMacro<'db>> {
        let mut seen = HashSet::new();
        let mut macros = Vec::new();

        for module in self.js_program(db).modules(db) {
            for src in module.macros(db) {
                let macro_ = self.lower_rs(db, *src);
                if seen.insert(macro_) {
                    macros.push(macro_);
                }
            }
        }

        macros.sort_by(|left, right| left.symbol(db).cmp(right.symbol(db)));
        macros
    }

    /// Whether any `js!` input or `rs` return value needs a JavaScript-owned Rust handle.
    pub fn uses_rust_handles(self, db: &'db dyn Db) -> bool {
        self.has_js_handle_inputs(db)
            || self
                .rs_macros(db)
                .iter()
                .any(|macro_| matches!(macro_.ret_type(db), RsType::RustHandle { .. }))
    }

    /// Whether converting `js!` inputs requires the bridge's Rust handle constructor.
    pub(crate) fn has_js_handle_inputs(self, db: &'db dyn Db) -> bool {
        self.js_macro_sources(db).iter().any(|site| {
            self.js_input_types(db, *site)
                .as_ref()
                .is_none_or(|inputs| {
                    inputs
                        .iter()
                        .any(|ty| matches!(ty, RsType::RustHandle { .. }))
                })
        })
    }

    /// All analyzed `js!` call sites in Rust dependencies and inline `rs` bodies.
    /// Sites with the same generated symbol retain their individual input types.
    #[salsa::tracked(returns(ref))]
    pub fn js_macro_sources(self, db: &'db dyn Db) -> Vec<JsMacroSrc<'db>> {
        self.rs_program(db)
            .modules_using_js(db)
            .iter()
            .flat_map(|module| module.js_macros(db))
            .chain(
                self.rs_macros(db)
                    .iter()
                    .flat_map(|site| site.js_macros(db)),
            )
            .copied()
            .collect()
    }

    /// Distinct `js!` implementations from all call sites, ordered by generated symbol.
    #[salsa::tracked(returns(ref))]
    pub fn js_macros(self, db: &'db dyn Db) -> Vec<JsMacro<'db>> {
        let mut seen = HashSet::new();
        let mut macros: Vec<_> = self
            .js_macro_sources(db)
            .iter()
            .filter_map(|src| self.lower_js(db, *src))
            .filter(|macro_| seen.insert(*macro_))
            .collect();
        macros.sort_unstable_by(|a, b| a.symbol(db).cmp(b.symbol(db)));
        macros
    }
}

/// The `rs` macro in js code.
#[salsa::interned]
pub struct RsMacro<'db> {
    program: RsProgram,
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
impl<'db> Runtime<'db> {
    /// Lowers this occurrence in the runtime's Rust dependency context.
    #[salsa::tracked]
    pub fn lower_rs(self, db: &'db dyn Db, src: RsMacroSrc<'db>) -> RsMacro<'db> {
        let body: Vec<_> = src
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

        RsMacro::new(db, self.rs_program(db), body)
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

    /// The resolved runtime signature, before choosing a wire representation.
    pub fn signature(self, db: &'db dyn Db) -> RsSignature<'db> {
        RsSignature {
            inputs: self.inputs(db),
            ret_type: self.ret_type(db),
            can_return_error: self.can_return_error(db),
        }
    }
}

/// Rust-facing argument and return types of a callable `rs` implementation.
pub struct RsSignature<'db> {
    pub inputs: &'db [RsType],
    pub ret_type: RsType,
    pub can_return_error: bool,
}

/// A shared `js!` implementation, independent of the Rust expressions at its call sites.
#[salsa::interned]
pub struct JsMacro<'db> {
    #[returns(ref)]
    pub body: String,
    pub input_count: usize,
}

#[salsa::tracked]
impl<'db> JsMacro<'db> {
    /// The link name used by both the proc macro and generated glue.
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        rustier_js_syntax::symbol(self.body(db))
    }
}

/// A Rust-side runtime type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum RsType {
    String,
    Bool,
    I8,
    I16,
    I32,
    U8,
    U16,
    U32,
    F32,
    F64,
    Void,
    JsValue,
    RustHandle { borrow: HandleBorrow },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum HandleBorrow {
    Owned,
    Shared,
    Mutable,
}

/// A js-side runtime type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum JsType {
    String,
    Boolean,
    Number,
    Undefined,
    Any,
    RustHandle,
}

impl RsType {
    /// Maps this runtime type to its JS-facing representation.
    pub fn js(&self) -> JsType {
        match self {
            Self::String => JsType::String,
            Self::Bool => JsType::Boolean,
            Self::Void => JsType::Undefined,
            Self::JsValue => JsType::Any,
            Self::RustHandle { .. } => JsType::RustHandle,
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
