//! Diagnostics surfaced during a build.

use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource};
#[cfg(feature = "napi-binding")]
use napi_derive::napi;

/// Source code span a diagnostic is anchored to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Span {
    /// `filename` interpretation depends on the originating crate: relative to
    /// the project root for the local crate, absolute under `~/.cargo/...` for
    /// registry deps, and so on.
    pub filename: PathBuf,
    /// 1-indexed `(line, column)` begin/end of the span.
    pub begin: (usize, usize),
    /// 1-indexed `(line, column)` begin/end of the span.
    pub end: (usize, usize),
}

/// Diagnostic label attached to a source code span.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct DiagLabel {
    pub message: Option<&'static str>,
    pub span: Span,
}

/// Structured diagnostic surfaced to JS.
#[salsa::accumulator]
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Diag {
    /// Code of the diagnostic, like `rust-curse::unresolved_import`.
    pub code: String,
    /// Primary leading message. Should explain the diagnostic as if no context
    /// was attached to it.
    pub message: String,
    /// Label attached to the diagnostic source code.
    pub label: Option<DiagLabel>,
    /// Help text to display to the user.
    pub help: Option<String>,
    /// Causes of the diagnostic.
    pub causes: Vec<Diag>,
}

pub type DiagResult<T> = Result<T, Vec<Diag>>;

impl Diag {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            label: None,
            help: None,
            causes: Vec::new(),
        }
    }

    /// Attaches a `Span` to the diagnostic.
    pub fn with_span(mut self, span: impl Into<Span>) -> Self {
        self.label = Some(DiagLabel {
            message: None,
            span: span.into(),
        });
        self
    }

    /// Attaches a `DiagLabel` to the diagnostic.
    pub fn with_label(mut self, label: DiagLabel) -> Self {
        self.label = Some(label);
        self
    }

    /// Attaches a help text to the diagnostic.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Attaches a cause to the diagnostic.
    pub fn with_cause(mut self, cause: Diag) -> Self {
        self.causes.push(cause);
        self
    }

    /// Attaches causes to the diagnostic.
    pub fn with_causes(mut self, causes: impl IntoIterator<Item = Diag>) -> Self {
        self.causes.extend(causes);
        self
    }

    /// Convert this `Diag` into a `DiagReport` for use with the diagnostic
    /// rendering machinery.
    ///
    /// Reads the source code relative to `root_abs_path`, in case the
    /// diagnostic refers to a source code file with relative paths.
    fn into_report(self, root_abs_path: &Path) -> DiagReport {
        let source_code = self.label.as_ref().and_then(|label| {
            let abs_path = if label.span.filename.is_absolute() {
                label.span.filename.clone()
            } else {
                root_abs_path.join(&label.span.filename)
            };

            std::fs::read_to_string(&abs_path)
                .ok()
                .map(|text| NamedSource::new(label.span.filename.to_string_lossy(), text))
        });

        let label = match (&self.label, &source_code) {
            (Some(label), Some(source_code)) => {
                let begin =
                    line_col_to_byte(source_code.inner(), label.span.begin.0, label.span.begin.1);
                let end = line_col_to_byte(source_code.inner(), label.span.end.0, label.span.end.1);
                Some(LabeledSpan::new(
                    label.message.map(|message| message.to_string()),
                    begin,
                    end - begin,
                ))
            }
            _ => None,
        };

        DiagReport {
            code: self.code,
            message: self.message,
            label,
            source_code,
            help: self.help,
            related: self
                .causes
                .into_iter()
                .map(|cause| cause.into_report(root_abs_path))
                .collect(),
        }
    }
}

/// Converts a line/column index to a byte offset in the source code.
fn line_col_to_byte(source: &str, line: usize, col: usize) -> usize {
    let line0 = line.saturating_sub(1);
    let col0 = col.saturating_sub(1);
    let mut byte = 0usize;
    for (i, l) in source.split_inclusive('\n').enumerate() {
        if i == line0 {
            let line_byte_len = l.trim_end_matches('\n').len();
            return byte + col0.min(line_byte_len);
        }
        byte += l.len();
    }
    source.len()
}

/// Internal diagnostic with [`miette::Diagnostic`] trait being implement for
/// it.
#[derive(Debug, Clone)]
struct DiagReport {
    pub code: String,
    pub message: String,
    pub label: Option<LabeledSpan>,
    pub source_code: Option<NamedSource<String>>,
    pub help: Option<String>,
    pub related: Vec<DiagReport>,
}

impl fmt::Display for DiagReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl Error for DiagReport {}

impl Diagnostic for DiagReport {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(&self.code))
    }

    fn severity(&self) -> Option<miette::Severity> {
        Some(miette::Severity::Warning)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        match &self.source_code {
            Some(code) => Some(code),
            None => None,
        }
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        match &self.label {
            Some(label) => Some(Box::new(std::iter::once(label.clone()))),
            None => None,
        }
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        match &self.help {
            Some(help) => Some(Box::new(help)),
            None => None,
        }
    }

    fn url<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        None
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        if self.related.is_empty() {
            None
        } else {
            Some(Box::new(
                self.related.iter().map(|report| report as &dyn Diagnostic),
            ))
        }
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        None
    }
}

impl DiagReport {
    fn render(&self, handler: &GraphicalReportHandler) -> Result<String, fmt::Error> {
        let mut rendered = String::new();
        handler.render_report(&mut rendered, self)?;
        Ok(rendered)
    }
}

/// Source code location a rendered diagnostic is anchored to.
#[cfg_attr(feature = "napi-binding", napi(object))]
#[derive(Clone)]
pub struct DiagLabelRendered {
    /// Message describing the labeled span, if any.
    pub message: Option<String>,
    /// Path to the source code file the diagnostic refers to.
    pub filename: String,
    /// 1-indexed line of the span start.
    pub begin_line: u32,
    /// 1-indexed column of the span start.
    pub begin_col: u32,
    /// 1-indexed line of the span end.
    pub end_line: u32,
    /// 1-indexed column of the span end.
    pub end_col: u32,
}

/// Rendered diagnostic message with nice rendered string.
#[cfg_attr(feature = "napi-binding", napi(object))]
#[derive(Clone)]
pub struct DiagRendered {
    /// Code of the diagnostic, like `rust-curse::unresolved_import`.
    pub code: String,
    /// Primary leading message. Should explain the diagnostic as if no context
    /// was attached to it.
    pub message: String,
    /// Source code location the diagnostic is anchored to, if any.
    pub label: Option<DiagLabelRendered>,
    /// Help text to display to the user.
    pub help: Option<String>,
    /// Causes of the diagnostic.
    pub causes: Vec<DiagRendered>,
    /// Rendered diagnostic message with syntax highlighting and source code
    /// context.
    pub rendered: Option<String>,
}

/// Theme for the rendered diagnostic message.
pub enum Theme {
    Unicode,
    UnicodeNoColor,
}

/// Render each diagnostic into a structured [`DiagRendered`] for the host.
pub fn render_diags(
    diags: impl IntoIterator<Item = Diag>,
    root_abs_path: &Path,
    theme: Theme,
) -> Result<Vec<DiagRendered>, fmt::Error> {
    let handler = match theme {
        Theme::Unicode => GraphicalReportHandler::new(),
        Theme::UnicodeNoColor => {
            GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
        }
    }
    .with_show_related_as_nested(true);

    diags
        .into_iter()
        .map(|diag| {
            let structured = structure_diag(&diag);
            let rendered = diag.into_report(root_abs_path).render(&handler)?;

            Ok(DiagRendered {
                rendered: Some(rendered),
                ..structured
            })
        })
        .collect()
}

fn structure_diag(diag: &Diag) -> DiagRendered {
    DiagRendered {
        code: diag.code.clone(),
        message: diag.message.clone(),
        label: diag.label.as_ref().map(|label| DiagLabelRendered {
            message: label.message.map(|message| message.to_string()),
            filename: label.span.filename.to_string_lossy().into_owned(),
            begin_line: label.span.begin.0 as u32,
            begin_col: label.span.begin.1 as u32,
            end_line: label.span.end.0 as u32,
            end_col: label.span.end.1 as u32,
        }),
        help: diag.help.clone(),
        causes: diag.causes.iter().map(structure_diag).collect(),
        rendered: None,
    }
}
