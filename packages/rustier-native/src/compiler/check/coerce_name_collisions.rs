use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::rust::{EnumType, Span, StructType},
};

#[salsa::tracked]
impl<'db> StructType<'db> {
    #[salsa::tracked]
    pub(super) fn check_coerce_name_collisions(self, db: &'db dyn Db) {
        let facade = self.facade(db);
        if facade.coerce(db).map(|coerce| coerce.name(db)) == Some("$of") {
            emit(db, self.def(db).span(db));
        }
    }
}

#[salsa::tracked]
impl<'db> EnumType<'db> {
    #[salsa::tracked]
    pub(super) fn check_coerce_name_collisions(self, db: &'db dyn Db) {
        let facade = self.facade(db);
        if facade.coerce(db).map(|coerce| coerce.name(db)) == Some("$of") {
            emit(db, self.def(db).span(db));
        }
    }
}

fn emit(db: &dyn Db, source_span: Option<&Span>) {
    CoerceNameCollision {
        source_code: source_span
            .map(|span| span.src(db).into())
            .unwrap_or_default(),
        span: source_span.map(|span| (span.offset, span.length).into()),
    }
    .accumulate(db);
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error(
    "public static method `of` conflicts with the builtin JS coerce method; the coerce method is renamed to `$of`"
)]
#[diagnostic(
    code(rustier::coerce_name_collision),
    help("call `$of` for the builtin object coerce method, or rename the static method"),
    severity(Warning)
)]
struct CoerceNameCollision {
    #[source_code]
    source_code: DiagSource,
    #[label("builtin coerce method renamed to `$of`")]
    span: Option<SourceSpan>,
}
