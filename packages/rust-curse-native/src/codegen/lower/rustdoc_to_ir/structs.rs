use miette::Diagnostic;
use rustdoc_types::{ItemEnum, ItemKind as RItemKind};
use thiserror::Error;

use crate::codegen::{
    db::Db,
    diag::{DiagAccumulateExt, DiagSource, DiagSpan},
    ir::{ItemId, ItemKind, ItemPath, Struct, StructId},
    rustdoc::RustdocId,
};

use super::{
    shapes::{RustdocShape, ShapeFieldLowerError, lower_shape},
    spans::RustdocSpanExt as _,
};

/// Lowers the rustdoc struct to the IR struct.
#[salsa::tracked]
pub fn lower_struct<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
) -> Result<Struct<'db>, Box<StructLowerError>> {
    let item = id.item(db);
    let crate_body = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Struct(def) => def,
        _ => unreachable!(),
    };

    let item_summary = crate_body.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, RItemKind::Struct);

    let mut causes = Vec::new();

    if !def.generics.params.is_empty() {
        causes.push(StructLowerError::UnsupportedGeneric);
    }

    if !causes.is_empty() {
        let crate_ = id.crate_(db);
        let source_code = item
            .span
            .as_ref()
            .map(|span| span.diag_source(db, crate_))
            .into();
        let span = item.span.as_ref().map(|span| span.diag_span(db, crate_));
        return Err(Box::new(StructLowerError::Skipped {
            struct_fqn: item_summary.path.join("::"),
            source_code,
            span,
            causes,
        }));
    }

    let (shape, field_skipped_causes) = lower_shape(db, id, RustdocShape::from(&def.kind));

    if !field_skipped_causes.is_empty() {
        let crate_ = id.crate_(db);
        let source_code = item
            .span
            .as_ref()
            .map(|span| span.diag_source(db, crate_))
            .into();
        let span = item.span.as_ref().map(|span| span.diag_span(db, crate_));
        StructLowerError::FieldsSkipped {
            struct_fqn: item_summary.path.join("::"),
            source_code,
            span,
            causes: field_skipped_causes,
        }
        .accumulate(db);
    }

    let docs = item.docs.clone();
    let span = item.span.as_ref().map(|span| span.lower(id.crate_(db)));
    let id = StructId::new(
        db,
        ItemId::new(db, ItemPath::new(db, &item_summary.path), ItemKind::Struct),
    )
    .expect("newly created struct item must have struct kind");

    Ok(Struct::new(db, id, shape, docs, span))
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum StructLowerError {
    #[error("the struct \"{struct_fqn}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_struct), severity(Warning))]
    Skipped {
        struct_fqn: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
        #[related]
        causes: Vec<StructLowerError>,
    },

    #[error("the struct \"{struct_fqn}\" has fields that cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_struct), severity(Warning))]
    FieldsSkipped {
        struct_fqn: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
        #[related]
        causes: Vec<ShapeFieldLowerError>,
    },

    #[error("generic structs are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_generic_struct),
        help(
            "export a non-generic wrapper that instantiates the generic parameters with concrete types"
        ),
        severity(Warning)
    )]
    UnsupportedGeneric,
}
