use crate::compiler::{
    db::Db,
    diag::{DiagSource, DiagSpan},
    ir::Span,
    rustdoc::Crate,
};

/// Converts rustdoc source spans into IR spans.
pub trait RustdocSpanExt {
    /// Lowers a rustdoc span into an IR span by resolving relative filenames
    /// against the crate directory.
    fn lower(&self, crate_: &Crate) -> Span;

    /// Loads and materializes the source file associated with this span for a
    /// Miette diagnostic.
    fn diag_source(&self, db: &dyn Db, crate_: &Crate) -> DiagSource {
        self.lower(crate_).diag_source(db)
    }

    /// Converts this span into a byte-offset Miette diagnostic span.
    fn diag_span(&self, db: &dyn Db, crate_: &Crate) -> DiagSpan {
        self.lower(crate_).diag_span(db)
    }
}

impl RustdocSpanExt for rustdoc_types::Span {
    fn lower(&self, crate_: &Crate) -> Span {
        Span {
            abs_path: if self.filename.is_absolute() {
                self.filename.clone()
            } else {
                crate_.dir.join(&self.filename)
            },
            src_path: self.filename.clone(),
            begin: self.begin,
            end: self.end,
        }
    }
}
