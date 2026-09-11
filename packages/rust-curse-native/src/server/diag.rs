use super::{ServerDiagnostic, ServerDiagnosticLocation, ServerDiagnosticSeverity};
use crate::compiler::diag::Diag;

impl From<Diag> for ServerDiagnostic {
    fn from(diag: Diag) -> Self {
        Self::from_diagnostic(diag.inner.as_ref())
    }
}

impl<D: miette::Diagnostic> From<D> for ServerDiagnostic {
    fn from(diagnostic: D) -> Self {
        Self::from_diagnostic(&diagnostic)
    }
}

impl ServerDiagnostic {
    fn from_diagnostic(inner: &dyn miette::Diagnostic) -> Self {
        let message = inner.to_string();
        let mut rendered = String::new();

        if miette::GraphicalReportHandler::new()
            .with_context_lines(0)
            .with_show_related_as_nested(true)
            .render_report(&mut rendered, inner)
            .is_err()
        {
            rendered = message.clone();
        }

        let mut diagnostic = Self {
            code: inner.code().map(|code| code.to_string()),
            severity: match inner.severity().unwrap_or(miette::Severity::Error) {
                miette::Severity::Error => ServerDiagnosticSeverity::Error,
                miette::Severity::Warning => ServerDiagnosticSeverity::Warning,
                miette::Severity::Advice => ServerDiagnosticSeverity::Advice,
            },
            file: None,
            loc: None,
            url: inner.url().map(|url| url.to_string()),
            rendered,
            message,
        };

        let label = inner.labels().and_then(|mut labels| {
            let first = labels.next()?;
            if first.primary() {
                Some(first)
            } else {
                Some(labels.find(|label| label.primary()).unwrap_or(first))
            }
        });

        if let (Some(source), Some(label)) = (inner.source_code(), label)
            && let Ok(contents) = source.read_span(label.inner(), 1, 1)
            && let Ok(text) = std::str::from_utf8(contents.data())
            && let Some(offset) = label.offset().checked_sub(contents.span().offset())
            && let Some(prefix) = text.get(..offset)
        {
            let line = contents.line() + prefix.bytes().filter(|&byte| byte == b'\n').count() + 1;
            let column = prefix
                .rsplit('\n')
                .next()
                .unwrap_or("")
                .encode_utf16()
                .count();

            if let (Ok(line), Ok(column)) = (u32::try_from(line), u32::try_from(column)) {
                diagnostic.file = contents.name().map(str::to_owned);
                diagnostic.loc = Some(ServerDiagnosticLocation {
                    file: diagnostic.file.clone(),
                    line,
                    column,
                });
            }
        }
        diagnostic
    }
}
