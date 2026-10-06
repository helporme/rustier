mod modules;

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::{
        rust::{FieldQualified, TypeKind},
        shim::ShimField,
    },
};

#[salsa::tracked]
impl<'db> FieldQualified<'db> {
    #[salsa::tracked]
    pub(super) fn lower_shim(self, db: &'db dyn Db) -> ShimField<'db> {
        let type_ = self._type(db);

        if matches!(type_.kind(db), TypeKind::Struct(_) | TypeKind::Enum(_))
            && type_.is_clonable(db) == Some(false)
        {
            let source_span = self.declaration(db).span(db);
            FieldReadonlyWarning {
                field_name: self.declaration(db).name(db).to_string(),
                field_type_fqn: type_.display(db).to_owned(),
                source_code: source_span
                    .map(|span| span.src(db).into())
                    .unwrap_or_default(),
                span: source_span.map(|span| (span.offset, span.length).into()),
            }
            .accumulate(db);
        }

        ShimField::new(db, self)
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error(
    "the field `{field_name}` is readonly in JS because \"{field_type_fqn}\" does not implement `Clone`"
)]
#[diagnostic(
    code(rust_curse::readonly_field),
    help(
        "derive or implement `Clone` for \"{field_type_fqn}\" to make this field writable from JS"
    ),
    severity(Warning)
)]
struct FieldReadonlyWarning {
    field_name: String,
    field_type_fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<SourceSpan>,
}
