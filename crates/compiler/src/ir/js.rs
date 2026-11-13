//! Starting IR representing the parsed JavaScript source files.

use crate::{db::Db, diag::Diag, ir::runtime::CompileError};
use miette::{Diagnostic, LabeledSpan, NamedSource};
use ra_ap_syntax::TextRange;
use salsa::Accumulator;

mod parsing;

/// The root input of the js program.
#[salsa::input]
pub struct JsProgram {
    #[returns(ref)]
    pub files: Vec<JsFile>,
}

impl JsProgram {
    pub fn check(self, db: &dyn Db) -> Vec<Diag> {
        check_program::accumulated::<Diag>(db, self)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Js modules present in the program.
    pub fn modules<'db>(self, db: &'db dyn Db) -> &'db Vec<JsModule<'db>> {
        modules(db, self)
    }
}

// seprate query for diag accumulation purpose, it's not possible on an inherent impl
#[salsa::tracked]
fn check_program(db: &dyn Db, program: JsProgram) {
    for module in program.modules(db) {
        for src in module.macros(db) {
            for diagnostic in src.check_compile_errors(db) {
                diagnostic.clone().accumulate(db);
            }
        }
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

#[salsa::tracked]
impl<'db> RsMacroSrc<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn check_compile_errors(self, db: &'db dyn Db) -> Vec<Diag> {
        let Some(errors) = self.as_macro(db).check_compile_errors(db) else {
            return vec![];
        };

        errors
            .iter()
            .map(|error| {
                let ranges = self.map_error_ranges(db, &error.ranges);
                SourceCompileError::new(db, self, error.clone(), merge_ranges(ranges)).into()
            })
            .collect()
    }

    fn map_error_ranges(
        self,
        db: &'db dyn Db,
        ranges: &[(usize, TextRange)],
    ) -> Vec<(usize, usize)> {
        ranges
            .iter()
            .map(|(part, offset)| match self.body(db)[*part] {
                JsPart::Text(range) => (
                    range.start(db) as usize + u32::from(offset.start()) as usize,
                    range.start(db) as usize + u32::from(offset.end()) as usize,
                ),
                JsPart::Interpolation { range, .. } => {
                    (range.start(db) as usize, range.end(db) as usize)
                }
            })
            .collect()
    }
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

#[derive(Clone, Debug, thiserror::Error, Diagnostic)]
#[error("{error}")]
#[diagnostic(forward(error))]
struct SourceCompileError {
    error: CompileError,
    #[source_code]
    source_code: NamedSource<String>,
    #[label(collection)]
    labels: Vec<LabeledSpan>,
}

impl SourceCompileError {
    fn new(
        db: &dyn Db,
        src: RsMacroSrc<'_>,
        error: CompileError,
        mut ranges: Vec<(usize, usize)>,
    ) -> Self {
        let span = src.range(db);
        let file = span.file(db);

        if ranges.is_empty() {
            ranges.push((span.start(db) as usize, span.end(db) as usize));
        }

        let labels = ranges
            .into_iter()
            .map(|(start, end)| LabeledSpan::new(Some(error.message.clone()), start, end - start))
            .collect();

        Self {
            error,
            source_code: NamedSource::new(file.module_id(db).clone(), file.contents(db).clone())
                .with_language("JavaScript"),
            labels,
        }
    }
}

fn merge_ranges(mut ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();

    for (start, end) in ranges {
        if let Some(last) = merged.last_mut().filter(|last| start <= last.1) {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }

    merged
}
