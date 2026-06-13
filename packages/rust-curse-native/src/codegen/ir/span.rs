use std::{path::PathBuf, sync::Arc};

use crate::tools::rustdoc::Crate;

/// Source code span.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct Span {
    /// Relative path to the source file from the project root.
    pub rel_path: PathBuf,
    /// Relative path to the source file from the project root.
    pub abs_path: PathBuf,
    pub source: Arc<String>,
    /// 1-indexed `(line, column)` begin/end of the span.
    pub begin: (usize, usize),
    /// 1-indexed `(line, column)` begin/end of the span.
    pub end: (usize, usize),
}

impl From<Span> for miette::SourceSpan {
    fn from(value: Span) -> Self {
        let source = value.source.as_str();

        let start = line_col_to_offset(source, value.begin);
        let mut stop = line_col_to_offset(source, value.end);

        if stop < source.len() {
            stop += source[stop..].chars().next().map_or(0, char::len_utf8);
        }

        (start, stop.saturating_sub(start)).into()
    }
}

fn line_col_to_offset(source: &str, (line, column): (usize, usize)) -> usize {
    let mut offset = 0;

    for (index, content) in source.split_inclusive('\n').enumerate() {
        if index + 1 != line {
            offset += content.len();
            continue;
        }

        let text = content.trim_end_matches(['\n', '\r']);
        return offset
            + text
                .char_indices()
                .nth(column.saturating_sub(1))
                .map_or(text.len(), |(byte, _)| byte);
    }

    source.len()
}

// The most convininet way of implementing `SourceCode` is to read the source
// file directly from the span.
impl miette::SourceCode for Span {
    fn read_span<'a>(
        &'a self,
        span: &miette::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        let source = self.source.as_bytes();
        let contents = source.read_span(span, context_lines_before, context_lines_after)?;

        Ok(Box::new(miette::MietteSpanContents::new_named(
            self.rel_path.display().to_string(),
            contents.data(),
            *contents.span(),
            contents.line(),
            contents.column(),
            contents.line_count(),
        )))
    }
}

/// Optinal source code span.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct OptionalSpan(pub Option<Span>);

impl From<Option<Span>> for OptionalSpan {
    fn from(value: Option<Span>) -> Self {
        Self(value)
    }
}

impl From<OptionalSpan> for miette::SourceSpan {
    fn from(value: OptionalSpan) -> Self {
        match value.0 {
            Some(span) => span.into(),
            None => (0, 0).into(),
        }
    }
}

impl miette::SourceCode for OptionalSpan {
    fn read_span<'a>(
        &'a self,
        span: &miette::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        match &self.0 {
            Some(_source) => _source.read_span(span, context_lines_before, context_lines_after),
            None => Ok(Box::new(miette::MietteSpanContents::new(
                b"/* source is not available */",
                *span,
                0,
                0,
                0,
            ))),
        }
    }
}

impl Crate {
    /// Converts the given [`rustdoc_types::Span`] to an [`ir::Span`] with an
    /// absolute filename.
    pub fn conv_span(&self, span: rustdoc_types::Span) -> Span {
        let abs_path = if span.filename.is_absolute() {
            span.filename.clone()
        } else {
            self.dir.join(span.filename.clone())
        };

        let source = std::fs::read_to_string(&abs_path).unwrap_or_default();

        Span {
            rel_path: if self.is_external {
                abs_path.clone()
            } else {
                span.filename
            },
            abs_path,
            source: Arc::new(source),
            begin: span.begin,
            end: span.end,
        }
    }
}
