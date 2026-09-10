use super::{ServerDiagnostic, ServerDiagnosticLocation, ServerDiagnosticSeverity};
use crate::compiler::diag::{Diag, Theme};

impl From<Diag> for ServerDiagnostic {
    fn from(diag: Diag) -> Self {
        let inner = diag.inner.as_ref();
        let message = inner.to_string();
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
            rendered: diag
                .render(Theme::Unicode)
                .unwrap_or_else(|_| message.clone()),
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

