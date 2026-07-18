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

/// Returns whether the enum implements [`Clone`].
#[salsa::tracked]
pub fn is_enum_clonable<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> bool {
    let item = id.item(db);
    let crate_body = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Enum(def) => def,
        _ => unreachable!(),
    };

    def.impls.iter().any(|impl_id| {
        let impl_item = crate_body
            .index
            .get(impl_id)
            .expect("enum impl must be present in the rustdoc index");
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

#[cfg(test)]
mod tests {
    use miette::Diagnostic;

    use super::*;
    use crate::codegen::{db::DbImpl, test::test_db};

    fn lower_test_enum(db: &DbImpl) -> Box<EnumLowerError> {
        lower_enum(db, db.rustdoc_id("TestEnum")).expect_err("enum lowering must fail")
    }

    fn diagnostic_code(diagnostic: &dyn Diagnostic) -> String {
        diagnostic
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "<missing code>".to_string())
    }

    fn related(diagnostic: &dyn Diagnostic) -> Vec<&dyn Diagnostic> {
        diagnostic
            .related()
            .map(|related| related.collect())
            .unwrap_or_default()
    }

    #[test]
    fn reports_unsupported_generic_enum() {
        let db = test_db! {
            pub enum TestEnum<T> {
                Value(T),
            }
        };

        let error = lower_test_enum(&db);
        assert_eq!(diagnostic_code(error.as_ref()), "rust_curse::skipped_enum");

        let causes = related(error.as_ref());
        assert_eq!(
            causes
                .iter()
                .map(|cause| diagnostic_code(*cause))
                .collect::<Vec<_>>(),
            [
                "rust_curse::unsupported_generic_enum",
                "rust_curse::skipped_enum_variant",
            ]
        );
    }

    #[test]
    fn reports_stripped_enum_variants() {
        let db = test_db! {
            pub enum TestEnum {
                Visible,
                #[doc(hidden)]
                Hidden,
            }
        };

        let error = lower_test_enum(&db);
        assert_eq!(diagnostic_code(error.as_ref()), "rust_curse::skipped_enum");

        let causes = related(error.as_ref());
        assert_eq!(causes.len(), 1);
        assert_eq!(
            diagnostic_code(causes[0]),
            "rust_curse::stripped_enum_variants"
        );
    }

    #[test]
    fn reports_enum_variant_with_unsupported_field() {
        let db = test_db! {
            pub enum TestEnum {
                Invalid(u64),
            }
        };

        let error = lower_test_enum(&db);
        assert_eq!(diagnostic_code(error.as_ref()), "rust_curse::skipped_enum");

        let enum_causes = related(error.as_ref());
        assert_eq!(enum_causes.len(), 1);
        assert_eq!(
            diagnostic_code(enum_causes[0]),
            "rust_curse::skipped_enum_variant"
        );

        let variant_causes = related(enum_causes[0]);
        assert_eq!(variant_causes.len(), 1);
        assert_eq!(
            diagnostic_code(variant_causes[0]),
            "rust_curse::skipped_field"
        );

        let field_causes = related(variant_causes[0]);
        assert_eq!(field_causes.len(), 1);
        assert_eq!(
            diagnostic_code(field_causes[0]),
            "rust_curse::unsupported_primitive"
        );
    }
}
