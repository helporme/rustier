use miette::NamedSource;

use crate::tools::rustdoc::Crate;

/// Source code span.
pub type Span = miette::SourceSpan;

/// Source code.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct SourceCode(pub Option<miette::NamedSource<String>>);

impl SourceCode {
    pub fn not_available() -> Self {
        Self(None)
    }
}

impl From<NamedSource<String>> for SourceCode {
    fn from(source: NamedSource<String>) -> Self {
        Self(Some(source))
    }
}

impl miette::SourceCode for SourceCode {
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

impl Crate {
    /// Converts the given [`rustdoc_types::Span`] to an [`Span`].
    pub fn read_span(&self, span: &rustdoc_types::Span) -> Span {
        let abs_path = if span.filename.is_absolute() {
            span.filename.clone()
        } else {
            self.dir.join(span.filename.clone())
        };

        let Ok(source) = std::fs::read_to_string(&abs_path) else {
            return (0, 0).into();
        };

        let offset = line_col_to_offset(&source, span.begin);
        let len = line_col_to_offset(&source, span.end).saturating_sub(offset);

        (offset, len).into()
    }

    /// Converts the given [`rustdoc_types::Span`] to an [`Span`], if available.
    pub fn read_span_opt(&self, span: Option<&rustdoc_types::Span>) -> Option<Span> {
        span.map(|s| self.read_span(s))
    }

    /// Reads the source code for the given [`rustdoc_types::Span`].
    pub fn read_source_code_for_span(&self, span: &rustdoc_types::Span) -> SourceCode {
        let abs_path = if span.filename.is_absolute() {
            span.filename.clone()
        } else {
            self.dir.join(span.filename.clone())
        };

        let source_name = span.filename.display().to_string();

        match std::fs::read_to_string(&abs_path) {
            Ok(source) => NamedSource::new(source_name, source).into(),
            Err(_) => SourceCode::not_available(),
        }
    }

    /// Reads the source code for the given [`rustdoc_types::Span`], if
    /// available.
    pub fn read_source_code_for_span_opt(&self, span: Option<&rustdoc_types::Span>) -> SourceCode {
        match span {
            Some(span) => self.read_source_code_for_span(span),
            None => SourceCode::not_available(),
        }
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
