//! Diagnostics surfaced during a build.

use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource};

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

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum Severity {
    Warning,
    Fatal,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Fatal => "fatal",
        }
    }
}

impl From<Severity> for miette::Severity {
    fn from(severity: Severity) -> Self {
        match severity {
            Severity::Warning => miette::Severity::Warning,
            Severity::Fatal => miette::Severity::Error,
        }
    }
}

/// Structured diagnostic.
#[salsa::accumulator]
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Diag {
    /// Code of the diagnostic, like `rust-curse::unresolved_import`.
    pub code: String,
    /// Primary leading message. Should explain the diagnostic as if no context
    /// was attached to it.
    pub message: String,
    /// Severity of the diagnostic.
    pub severity: Severity,
    /// Labels attached to the diagnostic source code.
    pub labels: Vec<DiagLabel>,
    /// Help text to display to the user.
    pub help: Option<String>,
    /// Causes of the diagnostic.
    pub causes: Vec<Diag>,
}

impl Diag {
    pub fn new(code: impl Into<String>, message: impl Into<String>, severity: Severity) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity,
            labels: Vec::new(),
            help: None,
            causes: Vec::new(),
        }
    }

    /// Adds a span to the diagnostic.
    pub fn add_span(self, span: impl Into<Span>) -> Self {
        self.add_label(DiagLabel {
            message: None,
            span: span.into(),
        })
    }

    /// Adds a span to the diagnostic, if the span is present.
    pub fn try_add_span(self, span: Option<impl Into<Span>>) -> Self {
        if let Some(span) = span {
            self.add_label(DiagLabel {
                message: None,
                span: span.into(),
            })
        } else {
            self
        }
    }

    /// Adds a labeled span to the diagnostic.
    pub fn add_label(mut self, label: DiagLabel) -> Self {
        self.labels.push(label);
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

    /// Attaches multiple causes to the diagnostic.
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
        let source_code = self.labels.iter().find_map(|label| {
            let abs_path = label_abs_path(label, root_abs_path);

            std::fs::read_to_string(&abs_path).ok().map(|text| {
                let display_name =
                    source_display_name(&abs_path, &label.span.filename, root_abs_path);
                (abs_path, NamedSource::new(display_name, text))
            })
        });

        let labels = match &source_code {
            Some((source_abs_path, source_code)) => self
                .labels
                .iter()
                .filter(|label| label_abs_path(label, root_abs_path) == *source_abs_path)
                .map(|label| {
                    let begin = line_col_to_byte(
                        source_code.inner(),
                        label.span.begin.0,
                        label.span.begin.1,
                    );
                    let end =
                        line_col_to_byte(source_code.inner(), label.span.end.0, label.span.end.1);
                    LabeledSpan::new(
                        label.message.map(|message| message.to_string()),
                        begin,
                        end.saturating_sub(begin),
                    )
                })
                .collect(),
            None => Vec::new(),
        };

        DiagReport {
            code: self.code,
            message: self.message,
            severity: self.severity,
            labels,
            source_code: source_code.map(|(_, source_code)| source_code),
            help: self.help,
            related: self
                .causes
                .into_iter()
                .map(|cause| cause.into_report(root_abs_path))
                .collect(),
        }
    }
}

fn label_abs_path(label: &DiagLabel, root_abs_path: &Path) -> PathBuf {
    if label.span.filename.is_absolute() {
        label.span.filename.clone()
    } else {
        root_abs_path.join(&label.span.filename)
    }
}

fn source_display_name(abs_path: &Path, span_filename: &Path, root_abs_path: &Path) -> String {
    if let Some(relative) = strip_non_empty_prefix(abs_path, root_abs_path) {
        return relative.to_string_lossy().into_owned();
    }

    if let (Ok(abs_path), Ok(root_abs_path)) =
        (abs_path.canonicalize(), root_abs_path.canonicalize())
    {
        if let Some(relative) = strip_non_empty_prefix(&abs_path, &root_abs_path) {
            return relative.to_string_lossy().into_owned();
        }
    }

    span_filename.to_string_lossy().into_owned()
}

fn strip_non_empty_prefix<'a>(path: &'a Path, prefix: &Path) -> Option<&'a Path> {
    let relative = path.strip_prefix(prefix).ok()?;
    (!relative.as_os_str().is_empty()).then_some(relative)
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
    pub severity: Severity,
    pub labels: Vec<LabeledSpan>,
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
        Some(self.severity.into())
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        match &self.source_code {
            Some(code) => Some(code),
            None => None,
        }
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        if self.labels.is_empty() {
            None
        } else {
            Some(Box::new(self.labels.iter().cloned()))
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

/// Rendered diagnostic message with nice rendered string, needed to pass the
/// diagnostic across the js/rust boundary.
// todo: once the rust-curse self-hosting is achieved, remove this DiagRender and use just normal
// Diag.
#[cfg_attr(feature = "napi-binding", napi_derive::napi(object))]
#[derive(Clone)]
pub struct DiagRendered {
    /// Code of the diagnostic, like `rust-curse::unresolved_import`.
    pub code: String,
    /// Primary leading message. Should explain the diagnostic as if no context
    /// was attached to it.
    pub message: String,
    /// Severity of the diagnostic.
    pub severity: String,
    /// Help text to display to the user.
    pub help: Option<String>,
    /// Causes of the diagnostic.
    pub causes: Vec<DiagRendered>,
    /// Rendered diagnostic message with syntax highlighting and source code
    /// context.
    pub rendered: Option<String>,
}

/// Theme for the rendered diagnostic message.
#[derive(Clone, Copy, Debug)]
pub enum Theme {
    Unicode,
    UnicodeNoColor,
}

/// Render each diagnostic into a [`DiagRendered`] that can be passed across the
/// js/rust boundary.
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
    .with_context_lines(0)
    .with_show_related_as_nested(true);

    diags
        .into_iter()
        .map(|diag| {
            let rendered = diag.clone().into_report(root_abs_path).render(&handler)?;

            Ok(DiagRendered {
                code: diag.code.clone(),
                message: diag.message.clone(),
                severity: diag.severity.as_str().to_string(),
                help: diag.help.clone(),
                rendered: Some(rendered),
                causes: render_diags(diag.causes, root_abs_path, theme)?,
            })
        })
        .collect()
}
