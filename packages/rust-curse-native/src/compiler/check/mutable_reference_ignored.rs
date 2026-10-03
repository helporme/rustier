use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::rust::{Callable, TypeKind},
};

#[salsa::tracked]
impl<'db> Callable<'db> {
    #[salsa::tracked]
    pub(super) fn check_mutable_reference_ignored(self, db: &'db dyn Db) {
        for param in self.params(db) {
            let TypeKind::Borrowed(reference) = param.type_(db).kind(db) else {
                continue;
            };
            if reference.is_mutable(db)
                && matches!(
                    reference.inner(db).kind(db),
                    TypeKind::bool
                        | TypeKind::char
                        | TypeKind::i8
                        | TypeKind::u8
                        | TypeKind::i16
                        | TypeKind::u16
                        | TypeKind::i32
                        | TypeKind::u32
                        | TypeKind::f32
                        | TypeKind::f64
                        | TypeKind::Str
                        | TypeKind::String
                )
            {
                let source_span = param.span(db).as_ref().or(self.declaration(db).span(db));
                MutableReferenceIgnored {
                    parameter: param.name(db).to_string(),
                    source_code: source_span
                        .map(|span| span.src(db).into())
                        .unwrap_or_default(),
                    span: source_span.map(|span| (span.offset, span.length).into()),
                }
                .accumulate(db);
            }
        }
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("writes through the mutable reference parameter `{parameter}` are not observable by JS")]
#[diagnostic(
    code(rust_curse::mutable_reference_ignored),
    help("pass the value by value or return the updated value instead"),
    severity(Warning)
)]
struct MutableReferenceIgnored {
    parameter: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<SourceSpan>,
}
