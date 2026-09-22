use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::{facade::FacadeClass, rust::TypeKind, shim::ShimClass},
};

pub(super) fn check<'db>(db: &'db dyn Db, class: ShimClass<'db>) {
    let name = match class.facade(db) {
        FacadeClass::Plain(class) => class.coerce(db).map(|coerce| coerce.name(db)),
        FacadeClass::Discriminated(class) => class.coerce(db).map(|coerce| coerce.name(db)),
    };
    if name != Some("$of") {
        return;
    }
    let source_span = match class.src_type(db).kind(db) {
        TypeKind::Struct(value) => value.def(db).span(db),
        TypeKind::Enum(value) => value.def(db).span(db),
        _ => unreachable!("classes represent structs or enums"),
    };
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
    code(rust_curse::coerce_name_collision),
    help("call `$of` for the builtin object coerce method, or rename the static method"),
    severity(Warning)
)]
struct CoerceNameCollision {
    #[source_code]
    source_code: DiagSource,
    #[label("builtin coerce method renamed to `$of`")]
    span: Option<SourceSpan>,
}
