//! Diagnostics surfaced during a build.

use std::{panic::RefUnwindSafe, sync::Arc as StdArc};

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme, MietteSpanContents, SourceCode};
use salsa::Accumulator;
use triomphe::Arc;

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct DiagSource(Option<Arc<str>>);

impl From<Arc<str>> for DiagSource {
    fn from(source: Arc<str>) -> Self {
        Self(Some(source))
    }
}

impl SourceCode for DiagSource {
    fn read_span<'a>(
        &'a self,
        span: &miette::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        let contents = self.0.as_deref().unwrap_or_default().read_span(
            span,
            context_lines_before,
            context_lines_after,
        )?;
        Ok(Box::new(
            MietteSpanContents::new(
                contents.data(),
                *contents.span(),
                contents.line(),
                contents.column(),
                contents.line_count(),
            )
            .with_language("Rust"),
        ))
    }
}

/// Diagnostic that can be accumulated into a salsa database.
#[salsa::accumulator]
#[derive(Clone)]
pub struct Diag {
    pub inner: StdArc<dyn Diagnostic + RefUnwindSafe + Send + Sync + 'static>,
}

impl Diag {
    /// Renders this diagnostic for display.
    pub fn render(&self, theme: Theme) -> Result<String, std::fmt::Error> {
        let handler = match theme {
            Theme::Unicode => GraphicalReportHandler::new(),
            Theme::UnicodeNoColor => {
                GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
                    .without_syntax_highlighting()
            }
        }
        .with_context_lines(0)
        .with_show_related_as_nested(true);

        let mut rendered = String::new();
        handler.render_report(&mut rendered, self.inner.as_ref())?;

        Ok(rendered)
    }
}

impl<T: Diagnostic + RefUnwindSafe + Send + Sync + 'static> From<T> for Diag {
    fn from(value: T) -> Self {
        let inner = StdArc::new(value);
        Self { inner }
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

#[derive(Clone, Copy)]
pub enum Theme {
    Unicode,
    UnicodeNoColor,
}
