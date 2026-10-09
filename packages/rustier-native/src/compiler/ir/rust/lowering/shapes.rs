use miette::{Diagnostic, SourceSpan};
use ra_ap_hir::StructKind;
use thiserror::Error;

use super::{FieldLowerError, diagnostic_location, lower_field};
use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::rust::{AsItem as _, FieldParent, Shape},
};

pub(super) fn lower_shape<'db>(
    db: &'db dyn Db,
    parent: FieldParent<'db>,
    kind: StructKind,
    fields: Vec<ra_ap_hir::Field>,
) -> Shape<'db> {
    let mut errors = Vec::new();
    let shape = match kind {
        StructKind::Unit => Shape::Unit,
        StructKind::Tuple => Shape::Tuple(
            fields
                .into_iter()
                .map(|field| match lower_field(db, field) {
                    Ok(field) => field,
                    Err(error) => {
                        if matches!(*error, FieldLowerError::Unsupported { .. }) {
                            errors.push(*error);
                        }

                        None
                    }
                })
                .collect(),
        ),
        StructKind::Record => {
            let field_count = fields.len();
            let fields: Vec<_> = fields
                .into_iter()
                .filter_map(|field| match lower_field(db, field) {
                    Ok(field) => field,
                    Err(error) => {
                        if matches!(*error, FieldLowerError::Unsupported { .. }) {
                            errors.push(*error);
                        }

                        None
                    }
                })
                .collect();
            Shape::Plain {
                has_stripped_fields: fields.len() != field_count,
                fields,
            }
        }
    };

    if !errors.is_empty() {
        match parent {
            FieldParent::Struct(struct_) => {
                let (source_code, span) = diagnostic_location(db, struct_.span(db));
                StructFieldsLowerError {
                    fqn: struct_.fqn(db).to_owned(),
                    source_code,
                    span,
                    causes: errors,
                }
                .accumulate(db)
            }
            FieldParent::Variant(variant) => {
                let (source_code, span) = diagnostic_location(db, variant.span(db));
                VariantFieldsLowerError {
                    fqn: variant.fqn(db).to_owned(),
                    source_code,
                    span,
                    causes: errors,
                }
                .accumulate(db)
            }
        }
    }

    shape
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("the struct \"{fqn}\" has fields that cannot be exported to JS")]
#[diagnostic(code(rustier::skipped_struct), severity(Warning))]
struct StructFieldsLowerError {
    fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<SourceSpan>,
    #[related]
    causes: Vec<super::FieldLowerError>,
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("the enum variant \"{fqn}\" has fields that cannot be exported to JS")]
#[diagnostic(code(rustier::skipped_enum_variant), severity(Warning))]
struct VariantFieldsLowerError {
    fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<SourceSpan>,
    #[related]
    causes: Vec<super::FieldLowerError>,
}
