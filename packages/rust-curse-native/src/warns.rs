//! Non-fatal diagnostics surfaced during a build.

use std::{
    error::Error,
    fmt::{self, Display},
    path::{Path, PathBuf},
};

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource};
#[cfg(feature = "napi-binding")]
use napi_derive::napi;

/// Warning label attached to a source code snippet.
#[derive(Debug, Clone)]
pub struct WarnLabel {
    pub message: Option<&'static str>,
    pub begin: (usize, usize),
    pub end: (usize, usize),
}

impl WarnLabel {
    pub fn new(message: Option<&'static str>, begin: (usize, usize), end: (usize, usize)) -> Self {
        Self {
            message,
            begin,
            end,
        }
    }
}

/// Structured warning surfaced to JS.
#[derive(Debug, Clone, Default)]
pub struct Warn {
    /// Code of the error, like `rust-curse::unresolved_import`.
    pub code: String,
    /// Primary leading message. Should explain the warning as if no context was
    /// attached to it.
    pub message: String,
    /// Path to the source code file.
    pub source_code_path: Option<PathBuf>,
    /// Label attached to the warning souce code.
    pub label: Option<WarnLabel>,
    /// Help text to display to the user.
    pub help: Option<String>,
}

impl Warn {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            source_code_path: None,
            label: None,
            help: None,
        }
    }

    pub fn source_code(mut self, path: impl Into<PathBuf>) -> Self {
        self.source_code_path = Some(path.into());
        self
    }

    pub fn label(mut self, label: WarnLabel) -> Self {
        self.label = Some(label);
        self
    }

    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

impl Warn {
    /// Convert this `Warn` into a `WarnDiagnostic` for use with the diagnostic
    /// rendering machinery.
    ///
    /// Reads the source code relative to `root_abs_path`, in case the warning
    /// refers to a source code file with relative paths.
    fn into_diagnostic(self, root_abs_path: &Path) -> WarnDiagnostic {
        if let Some(source_code_path) = &self.source_code_path {
            let abs_path = if source_code_path.is_absolute() {
                source_code_path.clone()
            } else {
                root_abs_path.join(source_code_path)
            };

            let source_code = std::fs::read_to_string(&abs_path)
                .ok()
                .map(|s| NamedSource::new(source_code_path.to_string_lossy(), s));

            let label = if let Some(source_code) = &source_code {
                self.label.as_ref().map(|l| {
                    let begin = line_col_to_byte(source_code.inner(), l.begin.0, l.begin.1);
                    let end = line_col_to_byte(source_code.inner(), l.end.0, l.end.1);
                    Some(LabeledSpan::new(
                        l.message.map(|s| s.to_string()),
                        begin,
                        end - begin,
                    ))
                })
            } else {
                None
            };

            WarnDiagnostic {
                code: self.code,
                message: self.message,
                label: label.flatten(),
                source_code,
                help: self.help.clone(),
            }
        } else {
            debug_assert!(
                self.label.is_none(),
                "label must be None when source_code_path is None"
            );

            WarnDiagnostic {
                code: self.code,
                message: self.message,
                help: self.help.clone(),
                label: None,
                source_code: None,
            }
        }
    }
}

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

/// Internal warn with [`miette::Diagnostic`] trait being implement for it.
#[derive(Debug, Clone)]
struct WarnDiagnostic {
    pub code: String,
    pub message: String,
    pub label: Option<LabeledSpan>,
    pub source_code: Option<NamedSource<String>>,
    pub help: Option<String>,
}

/// Rendered warning message with nice rendered string.
#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct WarnRendered {
    /// Code of the error, like `rust-curse::unresolved_import`.
    pub code: String,
    /// Primary leading message. Should explain the warning as if no context was
    /// attached to it.
    pub message: String,
    /// Rendered warning message with syntax highlighting and source code
    /// context.
    pub rendered: Option<String>,
}

/// Theme for the rendered warning message.
pub enum Theme {
    Unicode,
    UnicodeNoColor,
}

impl Display for WarnDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl Error for WarnDiagnostic {}

impl Diagnostic for WarnDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
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

    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        match &self.help {
            Some(help) => Some(Box::new(help)),
            None => None,
        }
    }

    fn url<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        None
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        None
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        None
    }
}

impl WarnDiagnostic {
    pub fn render(&self, handler: &GraphicalReportHandler) -> Result<WarnRendered, fmt::Error> {
        let mut rendered = String::new();
        handler.render_report(&mut rendered, self)?;

        Ok(WarnRendered {
            code: self.code.to_string(),
            message: self.message.to_string(),
            rendered: Some(rendered),
        })
    }
}

/// A collection of diagnostics emitted during a build.
pub type Warns = Vec<Warn>;

pub trait WarnsRenderer {
    fn render(self, root_abs_path: &Path, theme: Theme) -> Result<Vec<WarnRendered>, fmt::Error>;
}

impl WarnsRenderer for Warns {
    /// Render each diagnostic into a structured `Warn` for the host.
    fn render(self, root_abs_path: &Path, theme: Theme) -> Result<Vec<WarnRendered>, fmt::Error> {
        let handler = match theme {
            Theme::Unicode => GraphicalReportHandler::new(),
            Theme::UnicodeNoColor => {
                GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
            }
        };

        let mut rendered = Vec::new();
        for warn in self {
            rendered.push(warn.into_diagnostic(root_abs_path).render(&handler)?);
        }

        Ok(rendered)
    }
}
