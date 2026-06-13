//! Diagnostics surfaced during a build.

use std::{fmt, panic::RefUnwindSafe, sync::Arc};

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme};
use salsa::Accumulator;

#[derive(Clone, Copy)]
pub enum Theme {
    Unicode,
    UnicodeNoColor,
}

/// Diagnostic that can be accumulated into a salsa database.
///
/// It cannot be directly returned from the tracked function (for example, in
/// Result). Better return original diagnostic type instead.
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
pub trait DiagAccumulateExt {
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
