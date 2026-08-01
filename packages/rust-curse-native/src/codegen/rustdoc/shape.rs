use miette::Diagnostic;
use rustdoc_types::{Id, ItemEnum, StructKind, VariantKind};
use thiserror::Error;

use crate::codegen::{
    db::Db,
    diag::{DiagAccumulateExt, DiagSource, DiagSpan},
    ir::{
        item::ItemIdExt as _,
        shape::{Shape, ShapeField},
        type_::Type,
    },
    rustdoc::{
        CrateExt, ItemIdExt, RustdocId,
        enum_::is_enum_clonable,
        span::RustdocSpanExt as _,
        type_::{LoweredType, TypeLowerError, lower_type},
    },
};

/// A borrowed rustdoc field layout that can originate from a struct or enum
/// variant.
pub enum RustdocShape<'a> {
    Unit,
    Tuple(&'a [Option<Id>]),
    Plain {
        fields: &'a [Id],
        has_stripped_fields: bool,
    },
}

impl<'a> From<&'a StructKind> for RustdocShape<'a> {
    fn from(shape: &'a StructKind) -> Self {
        match shape {
            StructKind::Unit => Self::Unit,
            StructKind::Tuple(fields) => Self::Tuple(fields),
            StructKind::Plain {
                fields,
                has_stripped_fields,
            } => Self::Plain {
                fields,
                has_stripped_fields: *has_stripped_fields,
            },
        }
    }
}

impl<'a> From<&'a VariantKind> for RustdocShape<'a> {
    fn from(shape: &'a VariantKind) -> Self {
        match shape {
            VariantKind::Plain => Self::Unit,
            VariantKind::Tuple(fields) => Self::Tuple(fields),
            VariantKind::Struct {
                fields,
                has_stripped_fields,
            } => Self::Plain {
                fields,
                has_stripped_fields: *has_stripped_fields,
            },
        }
    }
}

/// Lowers a rustdoc struct or enum variant layout and returns any fields that
/// were skipped.
pub fn lower_shape<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
    shape: RustdocShape<'_>,
) -> (Shape<'db>, Vec<ShapeFieldLowerError>) {
    let crate_body = id.crate_body(db);

    match shape {
        RustdocShape::Unit => (Shape::Unit, Vec::new()),
        RustdocShape::Tuple(ids) => {
            let mut skipped_causes = Vec::new();
            let fields = ids
                .iter()
                .enumerate()
                .map(|(index, id)| {
                    let id = id.as_ref()?;
                    let id = crate_body.id(db, *id);
                    match lower_tuple_shape_field(db, id, index) {
                        Ok(field) => Some(field),
                        Err(diag) => {
                            skipped_causes.push(*diag);
                            None
                        }
                    }
                })
                .collect();

            (Shape::Tuple(fields), skipped_causes)
        }
        RustdocShape::Plain {
            fields,
            mut has_stripped_fields,
        } => {
            let mut lowered_fields = Vec::new();
            let mut skipped_causes = Vec::new();

            for id in fields {
                let id = crate_body.id(db, *id);
                match lower_plain_shape_field(db, id) {
                    Ok(field) => lowered_fields.push(field),
                    Err(diag) => {
                        has_stripped_fields = true;
                        skipped_causes.push(*diag);
                    }
                }
            }

            (
                Shape::Plain {
                    fields: lowered_fields,
                    has_stripped_fields,
                },
                skipped_causes,
            )
        }
    }
}

#[salsa::tracked]
fn lower_tuple_shape_field<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
    index: usize,
) -> Result<ShapeField<'db>, Box<ShapeFieldLowerError>> {
    let item = id.item(db);
    let field_type = lower_shape_field_type(db, id).map_err(|causes| {
        let crate_ = id.crate_(db);
        let source_code = item
            .span
            .as_ref()
            .map(|span| span.diag_source(db, crate_))
            .into();
        let span = item.span.as_ref().map(|span| span.diag_span(db, crate_));
        ShapeFieldLowerError::TupleFieldSkipped {
            source_code,
            span,
            causes,
        }
    })?;

    let name = index.to_string();
    let readonly_parent_fqn = match field_type {
        Type::Struct(struct_id) if !is_struct_clonable(db, struct_id.rustdoc_id(db)) => {
            Some(struct_id.fqn(db).to_owned())
        }
        Type::Enum(enum_id) if !is_enum_clonable(db, enum_id.rustdoc_id(db)) => {
            Some(enum_id.fqn(db).to_owned())
        }
        _ => None,
    };
    let is_readonly = readonly_parent_fqn.is_some();

    if let Some(parent_fqn) = readonly_parent_fqn {
        let crate_ = id.crate_(db);
        let source_code = item
            .span
            .as_ref()
            .map(|span| span.diag_source(db, crate_))
            .into();
        let span = item.span.as_ref().map(|span| span.diag_span(db, crate_));
        ShapeFieldReadonly::Tuple {
            parent_fqn,
            source_code,
            span,
        }
        .accumulate(db);
    }

    let docs = item.docs.clone();

    Ok(ShapeField::new(db, name, field_type, is_readonly, docs))
}

#[salsa::tracked]
fn lower_plain_shape_field<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
) -> Result<ShapeField<'db>, Box<ShapeFieldLowerError>> {
    let item = id.item(db);
    let name = item.name.clone().expect("shape field must have a name");

    let field_type = lower_shape_field_type(db, id).map_err(|causes| {
        let crate_ = id.crate_(db);
        let source_code = item
            .span
            .as_ref()
            .map(|span| span.diag_source(db, crate_))
            .into();
        let span = item.span.as_ref().map(|span| span.diag_span(db, crate_));
        ShapeFieldLowerError::PlainFieldSkipped {
            field_name: name.clone(),
            source_code,
            span,
            causes,
        }
    })?;

    let readonly_parent_fqn = match field_type {
        Type::Struct(struct_id) if !is_struct_clonable(db, struct_id.rustdoc_id(db)) => {
            Some(struct_id.fqn(db).to_owned())
        }
        Type::Enum(enum_id) if !is_enum_clonable(db, enum_id.rustdoc_id(db)) => {
            Some(enum_id.fqn(db).to_owned())
        }
        _ => None,
    };
    let is_readonly = readonly_parent_fqn.is_some();

    if let Some(parent_fqn) = readonly_parent_fqn {
        let crate_ = id.crate_(db);
        let source_code = item
            .span
            .as_ref()
            .map(|span| span.diag_source(db, crate_))
            .into();
        let span = item.span.as_ref().map(|span| span.diag_span(db, crate_));
        ShapeFieldReadonly::Plain {
            field_name: name.clone(),
            parent_fqn,
            source_code,
            span,
        }
        .accumulate(db);
    }

    let docs = item.docs.clone();

    Ok(ShapeField::new(db, name, field_type, is_readonly, docs))
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
fn lower_shape_field_type<'db>(
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
        unreachable!("field cannot be a borrowed type since generic structs are forbidden")
    };

    Ok(field_type)
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum ShapeFieldReadonly {
    #[error(
        "the field `{field_name}` is readonly in JS because \"{parent_fqn}\" does not implement `Clone`"
    )]
    #[diagnostic(
        code(rust_curse::readonly_field),
        help(
            "derive or implement `Clone` for \"{parent_fqn}\" to make this field writable from JS"
        ),
        severity(Warning)
    )]
    Plain {
        field_name: String,
        parent_fqn: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
    },

    #[error("the field is readonly in JS because \"{parent_fqn}\" does not implement `Clone`")]
    #[diagnostic(
        code(rust_curse::readonly_field),
        help(
            "derive or implement `Clone` for \"{parent_fqn}\" to make this field writable from JS"
        ),
        severity(Warning)
    )]
    Tuple {
        parent_fqn: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
    },
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum ShapeFieldLowerError {
    #[error("the field \"{field_name}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_field), severity(Warning))]
    PlainFieldSkipped {
        field_name: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
        #[related]
        causes: Vec<TypeLowerError>,
    },

    #[error("a field cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_field), severity(Warning))]
    TupleFieldSkipped {
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
        #[related]
        causes: Vec<TypeLowerError>,
    },
}
