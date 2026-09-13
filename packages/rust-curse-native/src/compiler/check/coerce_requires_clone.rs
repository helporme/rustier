use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::{rust::AsItem as _, shim::ShimClass},
};

pub(super) fn check<'db>(db: &'db dyn Db, class: ShimClass<'db>) {
    // Suggest Clone only when it is the remaining blocker for this ADT's
    // coerce method.
    let (fqn, source_span) = match class {
        ShimClass::Plain(class) => {
            let src = class.src(db);
            if src.is_clonable(db)
                || src.is_non_exhaustive(db)
                || src.shape(db).has_stripped_fields()
                || src.fields(db).any(|field| field.is_readonly(db))
            {
                return;
            }
            (src.fqn(db), src.span(db))
        }
        ShimClass::Discriminated(class) => {
            let src = class.src(db);
            if src.is_clonable(db)
                || !src.variants(db).iter().any(|variant| {
                    !variant.is_non_exhaustive(db)
                        && !variant.shape(db).has_stripped_fields()
                        && variant.fields(db).all(|field| !field.is_readonly(db))
                })
            {
                return;
            }
            (src.fqn(db), src.span(db))
        }
    };

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
    code(rust_curse::coerce_requires_clone),
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
