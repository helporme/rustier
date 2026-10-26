//! Starting IR representing the parsed JavaScript source files.

use crate::{db::Db, diag::Diag};

mod parsing;

/// The root input of the js program.
#[salsa::input]
pub struct JsProgram {
    #[returns(ref)]
    pub files: Vec<JsFile>,
}

impl JsProgram {
    /// Js modules present in the program.
    pub fn modules<'db>(self, db: &'db dyn Db) -> &'db Vec<JsModule<'db>> {
        modules(db, self)
    }

    /// Diagnostics accumulated while parsing the current program's modules.
    pub fn diagnostics(self, db: &dyn Db) -> Vec<&Diag> {
        modules::accumulated::<Diag>(db, self)
    }
}

#[salsa::tracked(returns(ref))]
fn modules<'db>(db: &'db dyn Db, program: JsProgram) -> Vec<JsModule<'db>> {
    program
        .files(db)
        .iter()
        .map(|file| file.module(db))
        .collect()
}

/// An immutable, interned source file snapshot.
#[salsa::interned(no_lifetime, debug)]
pub struct JsFile {
    /// e.g real/virtual file path.
    #[returns(ref)]
    pub module_id: String,
    /// The source text.
    #[returns(ref)]
    pub contents: String,
}

/// The parsed macro occurrences belonging to one source snapshot.
#[salsa::interned]
pub struct JsModule<'db> {
    /// The source snapshot from which this module was parsed.
    pub file: JsFile,
    /// Recognized `rs` tags in source order, including nested occurrences.
    #[returns(ref)]
    pub macros: Vec<RsMacroSrc<'db>>,
}

/// A half-open UTF-8 byte range within a particular source snapshot.
#[salsa::interned(debug)]
pub struct JsFileRange<'db> {
    /// The snapshot containing the range.
    pub file: JsFile,
    /// The inclusive starting byte offset.
    pub start: u32,
    /// The exclusive ending byte offset.
    pub end: u32,
}

/// A location-specific occurrence of an imported `rs` macro.
///
/// It's a source location of a `rs` macro call, but not [*the macro*](crate::ir::runtime::RsMacro) itself.
#[salsa::interned]
pub struct RsMacroSrc<'db> {
    /// The complete tagged expression, including its tag and template delimiters.
    pub range: JsFileRange<'db>,
    /// Raw template text interleaved with JS interpolation occurrences.
    #[returns(ref)]
    pub body: Vec<JsPart<'db>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum JsPart<'db> {
    /// A range of raw Rust text, without JS template escape decoding.
    Text(JsFileRange<'db>),
    Interpolation {
        /// The full `${...}` interpolation, including its delimiters.
        range: JsFileRange<'db>,
        /// The expression inside the delimiters, retaining whitespace and comments.
        expression: JsFileRange<'db>,
    },
}
