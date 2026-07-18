use miette::Diagnostic;
use rustdoc_types::{ItemEnum, ItemKind};
use thiserror::Error;

use crate::codegen::{
    db::Db,
    ir::{
        IrId,
        enum_::{Enum, EnumVariant},
        span::{SourceCode, Span},
    },
    rustdoc::{
        CrateExt, RustdocId,
        shape::{RustdocShape, ShapeFieldLowerError, lower_shape},
    },
};

/// Lowers the rustdoc enum to the IR enum.
#[salsa::tracked]
pub fn lower_enum<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
) -> Result<Enum<'db>, Box<EnumLowerError>> {
    let item = id.item(db);
    let crate_body = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Enum(def) => def,
        _ => unreachable!(),
    };

    let item_summary = crate_body.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, ItemKind::Enum);

    let mut causes = Vec::new();

    if !def.generics.params.is_empty() {
        causes.push(EnumLowerCause::UnsupportedGeneric);
    }

    if def.has_stripped_variants {
        causes.push(EnumLowerCause::StrippedVariants);
    }

    let mut variants = Vec::new();
    for variant_id in &def.variants {
        match lower_variant(db, crate_body.id(db, *variant_id)) {
            Ok(variant) => variants.push(variant),
            Err(error) => causes.push(EnumLowerCause::Variant(*error)),
        }
    }

    if !causes.is_empty() {
        let crate_ = id.crate_(db);
        return Err(Box::new(EnumLowerError::Skipped {
            enum_fqn: item_summary.path.join("::"),
            source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
            span: crate_.read_span_opt(item.span.as_ref()),
            causes,
        }));
    }

    let docs = item.docs.clone();
    let id = IrId::new(db, &item_summary.path);

    Ok(Enum::new(db, id, variants, docs))
}

/// Lowers a rustdoc enum variant to an IR enum variant.
fn lower_variant<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
) -> Result<EnumVariant<'db>, Box<EnumVariantLowerError>> {
    let item = id.item(db);
    let crate_body = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Variant(def) => def,
        _ => unreachable!(),
    };

    let item_summary = crate_body.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, ItemKind::Variant);

    let (shape, skipped_fields) = lower_shape(db, id, RustdocShape::from(&def.kind));
    if !skipped_fields.is_empty() {
        let crate_ = id.crate_(db);
        return Err(Box::new(EnumVariantLowerError::FieldsSkipped {
            variant_fqn: item_summary.path.join("::"),
            source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
            span: crate_.read_span_opt(item.span.as_ref()),
            causes: skipped_fields,
        }));
    }

    let name = item.name.clone().expect("enum variant must have a name");
    let discriminant = def
        .discriminant
        .as_ref()
        .map(|discriminant| discriminant.value.clone());
    let docs = item.docs.clone();

    Ok(EnumVariant::new(db, name, shape, discriminant, docs))
}

/// An error that prevents a rustdoc enum from being lowered to IR.
#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum EnumLowerError {
    #[error("the enum \"{enum_fqn}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_enum), severity(Warning))]
    Skipped {
        enum_fqn: String,
        #[source_code]
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
        #[related]
        causes: Vec<EnumLowerCause>,
    },
}

/// A specific reason why a rustdoc enum cannot be lowered to IR.
#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum EnumLowerCause {
    #[error("generic enums are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_generic_enum),
        help(
            "export a non-generic wrapper that instantiates the generic parameters with concrete types"
        ),
        severity(Warning)
    )]
    UnsupportedGeneric,

    #[error("enums with private or hidden variants are not supported")]
    #[diagnostic(
        code(rust_curse::stripped_enum_variants),
        help("make every variant public and remove any `#[doc(hidden)]` attributes"),
        severity(Warning)
    )]
    StrippedVariants,

    #[error(transparent)]
    #[diagnostic(transparent)]
    Variant(#[from] EnumVariantLowerError),
}

/// An error that prevents a rustdoc enum variant from being lowered to IR.
#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum EnumVariantLowerError {
    #[error("the enum variant \"{variant_fqn}\" has fields that cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_enum_variant), severity(Warning))]
    FieldsSkipped {
        variant_fqn: String,
        #[source_code]
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
        #[related]
        causes: Vec<ShapeFieldLowerError>,
    },
}
