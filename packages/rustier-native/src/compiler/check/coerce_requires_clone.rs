use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::rust::{AsItem as _, EnumType, Span, StructType},
};

#[salsa::tracked]
impl<'db> StructType<'db> {
    #[salsa::tracked]
    pub(super) fn check_coerce_requires_clone(self, db: &'db dyn Db) {
        if self.is_clonable(db) != Some(false)
            || self.def(db).is_non_exhaustive(db)
            || self.has_stripped_fields(db)
            || self.fields(db).any(|field| field.is_readonly(db))
        {
            return;
        }
        emit(db, self.fqn(db), self.def(db).span(db));
    }
}

#[salsa::tracked]
impl<'db> EnumType<'db> {
    #[salsa::tracked]
    pub(super) fn check_coerce_requires_clone(self, db: &'db dyn Db) {
        if self.is_clonable(db) != Some(false)
            || !self.variants(db).any(|variant| {
                !variant.declaration(db).is_non_exhaustive(db)
                    && !variant.has_stripped_fields(db)
                    && variant.fields(db).all(|field| !field.is_readonly(db))
            })
        {
            return;
        }
        emit(db, self.fqn(db), self.def(db).span(db));
    }
}

fn emit(db: &dyn Db, fqn: &str, source_span: Option<&Span>) {
    CoerceRequiresClone {
        fqn: fqn.to_owned(),
        source_code: source_span
            .map(|span| span.src(db).into())
            .unwrap_or_default(),
        span: source_span.map(|span| (span.offset, span.length).into()),
    }
    .accumulate(db);
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("implement `Clone` for \"{fqn}\" to make it coercible")]
#[diagnostic(
    code(rustier::coerce_requires_clone),
    help("add #[derive(Clone)] or implement Clone to enable the generated coerce method"),
    severity(Advice)
)]
struct CoerceRequiresClone {
    fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label("Clone is required for .of function")]
    span: Option<SourceSpan>,
}
