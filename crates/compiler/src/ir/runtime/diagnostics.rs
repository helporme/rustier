use super::{CompileError, Runtime};
use crate::{
    db::Db,
    diag::Diag,
    ir::js::{JsPart, RsMacroSrc},
};
use miette::{Diagnostic, LabeledSpan, NamedSource};
use ra_ap_syntax::TextRange;
use salsa::Accumulator;

#[salsa::tracked]
fn check_program(db: &dyn Db, runtime: Runtime<'_>) {
    for module in runtime.js_program(db).modules(db) {
        for src in module.macros(db) {
            for diagnostic in runtime.source_diagnostics(db, *src) {
                diagnostic.clone().accumulate(db);
            }
        }
    }
}

#[salsa::tracked]
impl<'db> Runtime<'db> {
    /// JavaScript parse diagnostics and Rust diagnostics for this pair of programs.
    pub fn check(self, db: &dyn Db) -> Vec<Diag> {
        check_program::accumulated::<Diag>(db, self)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Checks an `rs` occurrence and maps diagnostics to its JavaScript source.
    #[salsa::tracked(returns(ref))]
    pub fn source_diagnostics(self, db: &'db dyn Db, src: RsMacroSrc<'db>) -> Vec<Diag> {
        let Some(errors) = self.lower_rs(db, src).check_compile_errors(db) else {
            return vec![];
        };

        errors
            .iter()
            .map(|error| {
                let ranges = src.map_error_ranges(db, &error.ranges);
                SourceCompileError::new(db, src, error.clone(), merge_ranges(ranges)).into()
            })
            .collect()
    }
}

impl<'db> RsMacroSrc<'db> {
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
