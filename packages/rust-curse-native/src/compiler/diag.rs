//! Diagnostics surfaced during a build.

use std::{panic::RefUnwindSafe, sync::Arc as StdArc};

use miette::{Diagnostic, SourceCode};
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
        self.0.as_deref().unwrap_or_default().read_span(
            span,
            context_lines_before,
            context_lines_after,
        )
    }
}

/// Diagnostic that can be accumulated into a salsa database.
#[salsa::accumulator]
#[derive(Clone)]
pub struct Diag {
    pub inner: StdArc<dyn Diagnostic + RefUnwindSafe + Send + Sync + 'static>,
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
