use miette::Diagnostic;
use rustdoc_types::{ItemEnum, ItemKind};
use thiserror::Error;

use crate::codegen::{
    db::Db,
    diag::DiagAccumulateExt,
    ir::{
        IrId,
        span::{SourceCode, Span},
        struct_::Struct,
    },
    rustdoc::{
        RustdocId,
        shape::{RustdocShape, ShapeFieldLowerError, lower_shape},
    },
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
    assert_eq!(item_summary.kind, ItemKind::Struct);

    let mut causes = Vec::new();

    if !def.generics.params.is_empty() {
        causes.push(StructLowerError::UnsupportedGeneric);
    }

    if !causes.is_empty() {
        let crate_ = id.crate_(db);
        return Err(Box::new(StructLowerError::Skipped {
            struct_fqn: item_summary.path.join("::"),
            source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
            span: crate_.read_span_opt(item.span.as_ref()),
            causes,
        }));
    }

    let (shape, field_skipped_causes) = lower_shape(db, id, RustdocShape::from(&def.kind));

    if !field_skipped_causes.is_empty() {
        let crate_ = id.crate_(db);
        StructLowerError::FieldsSkipped {
            struct_fqn: item_summary.path.join("::"),
            source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
            span: crate_.read_span_opt(item.span.as_ref()),
            causes: field_skipped_causes,
        }
        .accumulate(db);
    }

    let docs = item.docs.clone();
    let id = IrId::new(db, &item_summary.path);

    Ok(Struct::new(db, id, shape, docs))
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum StructLowerError {
    #[error("the struct \"{struct_fqn}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_struct), severity(Warning))]
    Skipped {
        struct_fqn: String,
        #[source_code]
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
        #[related]
        causes: Vec<StructLowerError>,
    },

    #[error("the struct \"{struct_fqn}\" has fields that cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_struct), severity(Warning))]
    FieldsSkipped {
        struct_fqn: String,
        #[source_code]
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
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
