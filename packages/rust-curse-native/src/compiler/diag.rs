//! Diagnostics surfaced during a build.

use std::{fmt, panic::RefUnwindSafe, sync::Arc};

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme, NamedSource};
use salsa::Accumulator;

use crate::compiler::{db::Db, inputs::SourceFiles, ir::rust::Span};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct DiagSource(Option<NamedSource<Arc<str>>>);

impl DiagSource {
    /// Returns a diagnostic source placeholder for unavailable source files.
    pub fn not_available() -> Self {
        Self(None)
    }
}

impl From<Option<DiagSource>> for DiagSource {
    fn from(source: Option<DiagSource>) -> Self {
        source.unwrap_or_default()
    }
}

impl miette::SourceCode for DiagSource {
    fn read_span<'a>(
        &'a self,
        span: &miette::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        match &self.0 {
            Some(source) => source.read_span(span, context_lines_before, context_lines_after),
            None => Ok(Box::new(miette::MietteSpanContents::new(
                b"/* Not available */",
                (0, 0).into(),
                0,
                0,
                0,
            ))),
        }
    }
}

/// Miette byte-offset span used by rendered diagnostics.
pub type DiagSpan = miette::SourceSpan;

impl Span {
    /// Converts this row/column span into a byte-offset Miette span by loading
    /// its source file.
    pub fn diag_span(&self, db: &dyn Db) -> DiagSpan {
        let Ok(source_file) = SourceFiles::read(db, self.abs_path.as_path()) else {
            return (0, 0).into();
        };

        row_col_range_to_diag_span(&source_file.contents(db), self.begin, self.end)
    }

    /// Loads this span's source file and materializes a Miette-compatible
    /// source wrapper.
    pub fn diag_source(&self, db: &dyn Db) -> DiagSource {
        let Ok(source_file) = SourceFiles::read(db, self.abs_path.as_path()) else {
            return DiagSource::not_available();
        };

        DiagSource(Some(NamedSource::new(
            self.src_path.display().to_string(),
            source_file.contents(db),
        )))
    }
}

fn row_col_range_to_diag_span(
    source: &str,
    begin: (usize, usize),
    end: (usize, usize),
) -> DiagSpan {
    let start = line_col_to_byte_offset(source, begin);
    let end = line_col_to_byte_offset(source, end);
    (start, end.saturating_sub(start)).into()
}

fn line_col_to_byte_offset(source: &str, (row, column): (usize, usize)) -> usize {
    let mut lines = source.split_inclusive('\n');
    let row_start = lines
        .by_ref()
        .take(row.saturating_sub(1))
        .map(str::len)
        .sum::<usize>();
    let Some(line) = lines.next() else {
        return source.len();
    };
    let text = line.trim_end_matches(['\n', '\r']);

    row_start
        + text
            .char_indices()
            .nth(column.saturating_sub(1))
            .map_or(text.len(), |(byte, _)| byte)
}

#[derive(Clone, Copy)]
pub enum Theme {
    Unicode,
    UnicodeNoColor,
}

/// Diagnostic that can be accumulated into a salsa database.
#[salsa::accumulator]
#[derive(Clone)]
pub struct Diag {
    pub inner: Arc<dyn Diagnostic + RefUnwindSafe + Send + Sync + 'static>,
}

impl<T: Diagnostic + RefUnwindSafe + Send + Sync + 'static> From<T> for Diag {
    fn from(value: T) -> Self {
        let inner = Arc::new(value);
        Self { inner }
    }
}

impl Diag {
    /// Renders the diagnostic
    pub fn render(&self, theme: Theme) -> Result<String, fmt::Error> {
        let handler = match theme {
            Theme::Unicode => GraphicalReportHandler::new(),
            Theme::UnicodeNoColor => {
                GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
            }
        }
        .with_context_lines(0)
        .with_show_related_as_nested(true);

        let mut rendered = String::new();
        handler.render_report(&mut rendered, self.inner.as_ref())?;

        Ok(rendered)
    }
}

/// Extension trait for [`Diagnostic`] values that can be accumulated as
/// [`Diag`] into a salsa database.
pub(super) trait DiagAccumulateExt {
    fn accumulate<Db>(self, db: &Db)
    where
        Db: ?Sized + salsa::Database;
}

impl<T: Diagnostic + RefUnwindSafe + Send + Sync + 'static> DiagAccumulateExt for T {
    fn accumulate<Db>(self, db: &Db)
    where
        Db: ?Sized + salsa::Database,
    {
        Diag::from(self).accumulate(db);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_column_offsets_count_utf8_characters() {
        let source = "aé日\nnext";

        assert_eq!(line_col_to_byte_offset(source, (1, 1)), 0);
        assert_eq!(line_col_to_byte_offset(source, (1, 2)), 1);
        assert_eq!(line_col_to_byte_offset(source, (1, 3)), 3);
        assert_eq!(line_col_to_byte_offset(source, (1, 4)), 6);
    }

    #[test]
    fn line_column_offsets_handle_lf_and_crlf() {
        let source = "one\r\ntwo\nthree";

        assert_eq!(line_col_to_byte_offset(source, (2, 1)), 5);
        assert_eq!(line_col_to_byte_offset(source, (3, 1)), 9);
    }

    #[test]
    fn line_column_offsets_clamp_beyond_available_source() {
        let source = "one\ntwo";

        assert_eq!(line_col_to_byte_offset(source, (9, 1)), source.len());
        assert_eq!(line_col_to_byte_offset(source, (1, 9)), 3);
    }
}
