use std::panic;

use miette::Diagnostic;
use rustdoc_types::{ItemEnum, ItemKind, StructKind};
use thiserror::Error;

use crate::codegen::{
    db::Db,
    diag::DiagAccumulateExt,
    ir::{
        self,
        span::{SourceCode, Span},
        type_::Type,
    },
    rustdoc::{
        type_::{lower_type, LoweredType, TypeLowerError},
        CrateExt, RustdocId,
    },
};

/// Lowers the rustdoc struct to the IR struct.
#[salsa::tracked]
pub fn lower_struct<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
) -> Result<ir::struct_::Struct<'db>, Box<StructLowerError>> {
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
        StructKind::Unit => (ir::struct_::StructShape::Unit, vec![]),
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

            (
                ir::struct_::StructShape::Tuple(tuple_fields),
                skipped_causes,
            )
        }
        StructKind::Plain {
            fields,
            mut has_stripped_fields,
        } => {
            let mut plain_fields: Vec<ir::struct_::StructField<'db>> = vec![];
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
                ir::struct_::StructShape::Plain {
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
    let id = ir::IrId::new(db, &item_summary.path);

    Ok(ir::struct_::Struct::new(db, id, shape, docs))
}

#[salsa::tracked]
fn lower_tuple_struct_field<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
    index: usize,
) -> Result<ir::struct_::StructField<'db>, Box<StructFieldLowerError>> {
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
    let readonly_struct_fqn = match field_type {
        Type::Struct(struct_id) if !is_struct_clonable(db, struct_id.rustdoc_id(db)) => {
            Some(struct_id.path(db).join("::"))
        }
        _ => None,
    };
    let is_readonly = readonly_struct_fqn.is_some();

    if let Some(struct_fqn) = readonly_struct_fqn {
        let crate_ = id.crate_(db);
        StructFieldReadonly::Tuple {
            struct_fqn,
            source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
            span: crate_.read_span_opt(item.span.as_ref()),
        }
        .accumulate(db);
    }

    let docs = item.docs.clone();

    Ok(ir::struct_::StructField::new(
        db,
        name,
        field_type,
        is_readonly,
        docs,
    ))
}

#[salsa::tracked]
fn lower_plain_struct_field<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
) -> Result<ir::struct_::StructField<'db>, Box<StructFieldLowerError>> {
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

    let readonly_struct_fqn = match field_type {
        Type::Struct(struct_id) if !is_struct_clonable(db, struct_id.rustdoc_id(db)) => {
            Some(struct_id.path(db).join("::"))
        }
        _ => None,
    };
    let is_readonly = readonly_struct_fqn.is_some();

    if let Some(struct_fqn) = readonly_struct_fqn {
        let crate_ = id.crate_(db);
        StructFieldReadonly::Plain {
            field_name: name.clone(),
            struct_fqn,
            source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
            span: crate_.read_span_opt(item.span.as_ref()),
        }
        .accumulate(db);
    }

    let docs = item.docs.clone();

    Ok(ir::struct_::StructField::new(
        db,
        name,
        field_type,
        is_readonly,
        docs,
    ))
}

/// Returns whether the struct implements [`Clone`].
#[salsa::tracked]
pub fn is_struct_clonable<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> bool {
    let item = id.item(db);
    let crate_body = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Struct(def) => def,
        _ => unreachable!(),
    };

    def.impls.iter().any(|impl_id| {
        let impl_item = crate_body
            .index
            .get(impl_id)
            .expect("struct impl must be present in the rustdoc index");
        let ItemEnum::Impl(impl_) = &impl_item.inner else {
            unreachable!()
        };
        if impl_.is_negative {
            return false;
        }
        let Some(trait_) = &impl_.trait_ else {
            return false;
        };
        let Some(trait_summary) = crate_body.paths.get(&trait_.id) else {
            return false;
        };

        trait_summary.path == ["core", "clone", "Clone"]
    })
}

#[salsa::tracked]
fn lower_struct_field_type<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
) -> Result<Type<'db>, Vec<TypeLowerError>> {
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
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
        #[related]
        causes: Vec<StructLowerError>,
    },

    #[error("the struct \"{struct_fqn}\" has fields that cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_struct), severity(Warning))]
    FieldsSkipped {
        /// The fully qualified name of the struct being lowered.
        struct_fqn: String,
        #[source_code]
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
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
pub enum StructFieldReadonly {
    #[error("the field `{field_name}` is readonly in JS because \"{struct_fqn}\" does not implement `Clone`")]
    #[diagnostic(
        code(rust_curse::readonly_struct_field),
        help(
            "derive or implement `Clone` for \"{struct_fqn}\" to make this field writable from JS"
        ),
        severity(Warning)
    )]
    Plain {
        field_name: String,
        /// The fully qualified name of the field's struct.
        struct_fqn: String,
        #[source_code]
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
    },

    #[error("the field is readonly in JS because \"{struct_fqn}\" does not implement `Clone`")]
    #[diagnostic(
        code(rust_curse::readonly_struct_field),
        help(
            "derive or implement `Clone` for \"{struct_fqn}\" to make this field writable from JS"
        ),
        severity(Warning)
    )]
    Tuple {
        /// The fully qualified name of the field's struct.
        struct_fqn: String,
        #[source_code]
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
    },
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum StructFieldLowerError {
    #[error("the field \"{field_name}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_field), severity(Warning))]
    PlainFieldSkipped {
        field_name: String,
        #[source_code]
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
        #[related]
        causes: Vec<TypeLowerError>,
    },

    #[error("a field cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_field), severity(Warning))]
    TupleFieldSkipped {
        #[source_code]
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
        #[related]
        causes: Vec<TypeLowerError>,
    },
}
