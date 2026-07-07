use std::panic;

use miette::Diagnostic;
use rustdoc_types::{ItemEnum, ItemKind, StructKind};
use thiserror::Error;

use crate::codegen::{
    db::Db,
    diag::DiagAccumulateExt,
    ir,
    rustdoc::{lower_type, CrateExt, Id, LoweredType, TypeLowerError},
};

#[salsa::tracked]
pub fn lower_struct<'db>(
    db: &'db dyn Db,
    id: Id<'db>,
) -> Result<ir::Struct<'db>, Box<StructLowerError>> {
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

    let (shape, field_skipped_causes) = match &def.kind {
        StructKind::Unit => (ir::StructShape::Unit, vec![]),
        StructKind::Tuple(ids) => {
            let mut skipped_causes = Vec::new();

            let tuple_fields = ids
                .iter()
                .enumerate()
                .map(|(index, id)| {
                    id.map(|id| {
                        let id = crate_body.id(db, id);
                        let field = lower_tuple_struct_field(db, id, index);
                        match field {
                            Ok(ty) => Some(ty),
                            Err(diag) => {
                                skipped_causes.push(*diag);
                                None
                            }
                        }
                    })
                    .flatten()
                })
                .collect::<Vec<_>>();

            (ir::StructShape::Tuple(tuple_fields), skipped_causes)
        }
        StructKind::Plain {
            fields,
            mut has_stripped_fields,
        } => {
            let mut plain_fields: Vec<ir::StructField<'db>> = vec![];
            let mut skipped_causes = Vec::new();

            for id in fields {
                let id = crate_body.id(db, *id);
                let field = lower_plain_struct_field(db, id);

                match field {
                    Ok(field) => {
                        plain_fields.push(field);
                    }
                    Err(diag) => {
                        has_stripped_fields = true;
                        skipped_causes.push(*diag);
                    }
                }
            }

            (
                ir::StructShape::Plain {
                    fields: plain_fields,
                    has_stripped_fields,
                },
                skipped_causes,
            )
        }
    };

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
    let id = ir::Id::new(db, &item_summary.path);

    Ok(ir::Struct::new(db, id, shape, docs))
}

#[salsa::tracked]
fn lower_tuple_struct_field<'db>(
    db: &'db dyn Db,
    id: Id<'db>,
    index: usize,
) -> Result<ir::StructField<'db>, Box<StructFieldLowerError>> {
    let item = id.item(db);
    let field_type = lower_struct_field_type(db, id).map_err(|causes| {
        let crate_ = id.crate_(db);
        StructFieldLowerError::TupleFieldSkipped {
            source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
            span: crate_.read_span_opt(item.span.as_ref()),
            causes,
        }
    })?;

    let name = index.to_string();
    let docs = item.docs.clone();

    Ok(ir::StructField::new(db, name, field_type, docs))
}

#[salsa::tracked]
fn lower_plain_struct_field<'db>(
    db: &'db dyn Db,
    id: Id<'db>,
) -> Result<ir::StructField<'db>, Box<StructFieldLowerError>> {
    let item = id.item(db);
    let name = item.name.clone().expect("struct field must have a name");

    let field_type = lower_struct_field_type(db, id).map_err(|causes| {
        let crate_ = id.crate_(db);
        StructFieldLowerError::PlainFieldSkipped {
            field_name: name.clone(),
            source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
            span: crate_.read_span_opt(item.span.as_ref()),
            causes,
        }
    })?;

    let docs = item.docs.clone();

    Ok(ir::StructField::new(db, name, field_type, docs))
}

#[salsa::tracked]
fn lower_struct_field_type<'db>(
    db: &'db dyn Db,
    id: Id<'db>,
) -> Result<ir::Type, Vec<TypeLowerError>> {
    let item = id.item(db);
    let crate_body = id.crate_body(db);

    let field_type = match &item.inner {
        ItemEnum::StructField(def) => def,
        _ => unreachable!(),
    };
    let field_type = crate_body.ty(db, field_type);
    let field_type = lower_type(db, field_type)?;

    let LoweredType::Moved(field_type) = field_type else {
        unreachable!("param cannot be a borrowed type since generic structs are forbidden")
    };

    Ok(field_type)
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum StructLowerError {
    #[error("the struct \"{struct_fqn}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_struct), severity(Warning))]
    Skipped {
        /// The fully qualified name of the struct being lowered.
        struct_fqn: String,
        #[source_code]
        source_code: ir::SourceCode,
        #[label]
        span: Option<ir::Span>,
        #[related]
        causes: Vec<StructLowerError>,
    },

    #[error("the struct \"{struct_fqn}\" has fields that cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_struct), severity(Warning))]
    FieldsSkipped {
        /// The fully qualified name of the struct being lowered.
        struct_fqn: String,
        #[source_code]
        source_code: ir::SourceCode,
        #[label]
        span: Option<ir::Span>,
        #[related]
        causes: Vec<StructFieldLowerError>,
    },

    #[error("generic structs are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_generic_struct),
        help("export a non-generic wrapper that instantiates the generic parameters with concrete types"),
        severity(Warning)
    )]
    UnsupportedGeneric,
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum StructFieldLowerError {
    #[error("the field \"{field_name}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_field), severity(Warning))]
    PlainFieldSkipped {
        field_name: String,
        #[source_code]
        source_code: ir::SourceCode,
        #[label]
        span: Option<ir::Span>,
        #[related]
        causes: Vec<TypeLowerError>,
    },

    #[error("a field cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_field), severity(Warning))]
    TupleFieldSkipped {
        #[source_code]
        source_code: ir::SourceCode,
        #[label]
        span: Option<ir::Span>,
        #[related]
        causes: Vec<TypeLowerError>,
    },
}
