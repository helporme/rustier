use miette::Diagnostic;
use rustdoc_types::{Attribute, ItemEnum, ItemKind as RsItemKind, VariantKind};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource, DiagSpan},
    ir::{
        CLikeEnum, CLikeEnumId, CLikeEnumVariant, Enum, EnumId, EnumVariant, ItemId, ItemKind,
        ItemPath,
    },
    rustdoc::{CrateExt, RustdocId},
};

use super::{
    shapes::{RustdocShape, ShapeFieldLowerError, lower_shape},
    spans::RustdocSpanExt as _,
};

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, salsa::Update)]
pub enum LoweredEnum<'db> {
    Enum(Enum<'db>),
    CLikeEnum(CLikeEnum<'db>),
}

/// Lowers a rustdoc enum to either a data-carrying or C-like IR enum.
#[salsa::tracked]
pub fn lower_enum<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
) -> Result<LoweredEnum<'db>, Box<EnumLowerError>> {
    let item = id.item(db);
    let crate_body = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Enum(def) => def,
        _ => unreachable!(),
    };

    let item_summary = crate_body.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, RsItemKind::Enum);

    let mut causes = Vec::new();
    if !def.generics.params.is_empty() {
        causes.push(EnumLowerCause::UnsupportedGeneric);
    }
    if def.has_stripped_variants {
        causes.push(EnumLowerCause::StrippedVariants);
    }

    let is_c_like = def.variants.iter().all(|variant_id| {
        let variant_item = crate_body
            .index
            .get(variant_id)
            .expect("enum variant must be present in the rustdoc index");
        let ItemEnum::Variant(variant) = &variant_item.inner else {
            unreachable!()
        };

        matches!(variant.kind, VariantKind::Plain)
    });

    if is_c_like {
        lower_c_like_enum(db, id, causes).map(LoweredEnum::CLikeEnum)
    } else {
        lower_data_carrying_enum(db, id, causes).map(LoweredEnum::Enum)
    }
}

fn lower_data_carrying_enum<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
    mut causes: Vec<EnumLowerCause>,
) -> Result<Enum<'db>, Box<EnumLowerError>> {
    let item = id.item(db);
    let crate_body = id.crate_body(db);
    let def = match &item.inner {
        ItemEnum::Enum(def) => def,
        _ => unreachable!(),
    };

    let mut variants = Vec::new();
    for variant_id in &def.variants {
        match lower_variant(db, crate_body.id(db, *variant_id)) {
            Ok(variant) => variants.push(variant),
            Err(error) => causes.push(EnumLowerCause::Variant(*error)),
        }
    }

    if !causes.is_empty() {
        return Err(enum_lower_error(db, id, causes));
    }

    let item_summary = crate_body.paths.get(&item.id).unwrap();
    let span = item.span.as_ref().map(|span| span.lower(id.crate_(db)));
    let id = EnumId::new(
        db,
        ItemId::new(db, ItemPath::new(db, &item_summary.path), ItemKind::Enum),
    )
    .expect("newly created enum item must have enum kind");

    Ok(Enum::new(
        db,
        id,
        variants,
        item.docs.clone(),
        span,
        item.attrs.contains(&Attribute::NonExhaustive),
    ))
}

fn lower_c_like_enum<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
    causes: Vec<EnumLowerCause>,
) -> Result<CLikeEnum<'db>, Box<EnumLowerError>> {
    if !causes.is_empty() {
        return Err(enum_lower_error(db, id, causes));
    }

    let item = id.item(db);
    let crate_body = id.crate_body(db);
    let def = match &item.inner {
        ItemEnum::Enum(def) => def,
        _ => unreachable!(),
    };

    let variants = def
        .variants
        .iter()
        .map(|variant_id| {
            let variant_item = crate_body
                .index
                .get(variant_id)
                .expect("enum variant must be present in the rustdoc index");
            let variant = match &variant_item.inner {
                ItemEnum::Variant(variant) => variant,
                _ => unreachable!(),
            };
            let name = variant_item
                .name
                .clone()
                .expect("enum variant must have a name");
            let discriminant = variant
                .discriminant
                .as_ref()
                .map(|discriminant| discriminant.value.clone());

            let span = variant_item
                .span
                .as_ref()
                .map(|span| span.lower(id.crate_(db)));

            CLikeEnumVariant::new(db, name, discriminant, variant_item.docs.clone(), span)
        })
        .collect();

    let item_summary = crate_body.paths.get(&item.id).unwrap();
    let span = item.span.as_ref().map(|span| span.lower(id.crate_(db)));
    let id = ItemId::new(
        db,
        ItemPath::new(db, &item_summary.path),
        ItemKind::CLikeEnum,
    );
    let id = CLikeEnumId::new(db, id).unwrap();
    Ok(CLikeEnum::new(db, id, variants, item.docs.clone(), span))
}

fn enum_lower_error(
    db: &dyn Db,
    id: RustdocId<'_>,
    causes: Vec<EnumLowerCause>,
) -> Box<EnumLowerError> {
    let item = id.item(db);
    let item_summary = id.crate_body(db).paths.get(&item.id).unwrap();
    let crate_ = id.crate_(db);
    let source_code = item
        .span
        .as_ref()
        .map(|span| span.diag_source(db, crate_))
        .into();
    let span = item.span.as_ref().map(|span| span.diag_span(db, crate_));

    Box::new(EnumLowerError::Skipped {
        enum_fqn: item_summary.path.join("::"),
        source_code,
        span,
        causes,
    })
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
    assert_eq!(item_summary.kind, RsItemKind::Variant);

    let (shape, skipped_fields) = lower_shape(db, id, RustdocShape::from(&def.kind));
    if !skipped_fields.is_empty() {
        let crate_ = id.crate_(db);
        let source_code = item
            .span
            .as_ref()
            .map(|span| span.diag_source(db, crate_))
            .into();
        let span = item.span.as_ref().map(|span| span.diag_span(db, crate_));
        return Err(Box::new(EnumVariantLowerError::FieldsSkipped {
            variant_fqn: item_summary.path.join("::"),
            source_code,
            span,
            causes: skipped_fields,
        }));
    }

    if !matches!(def.kind, VariantKind::Plain) && def.discriminant.is_some() {
        let crate_ = id.crate_(db);
        let source_code = item
            .span
            .as_ref()
            .map(|span| span.diag_source(db, crate_))
            .into();
        let span = item.span.as_ref().map(|span| span.diag_span(db, crate_));
        IgnoredNonUnitVariantDiscriminant {
            variant_fqn: item_summary.path.join("::"),
            source_code,
            span,
        }
        .accumulate(db);
    }

    let name = item.name.clone().expect("enum variant must have a name");
    let docs = item.docs.clone();
    let span = item.span.as_ref().map(|span| span.lower(id.crate_(db)));

    Ok(EnumVariant::new(db, name, shape, docs, span))
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

/// Warns that an explicit discriminant on a data-carrying variant is not
/// represented in IR.
#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("the discriminant assigned to non-unit enum variant \"{variant_fqn}\" is ignored")]
#[diagnostic(
    code(rust_curse::ignored_non_unit_variant_discriminant),
    severity(Warning)
)]
pub struct IgnoredNonUnitVariantDiscriminant {
    variant_fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<DiagSpan>,
}

/// An error that prevents a rustdoc enum from being lowered to IR.
#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum EnumLowerError {
    #[error("the enum \"{enum_fqn}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_enum), severity(Warning))]
    Skipped {
        enum_fqn: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
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
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
        #[related]
        causes: Vec<ShapeFieldLowerError>,
    },
}

#[cfg(test)]
mod tests {
    use miette::Diagnostic;

    use super::*;
    use crate::compiler::{db::DbImpl, diag::Diag, test::test_db};

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

    #[test]
    fn lowers_unit_only_enum_to_c_like_enum() {
        let db = test_db! {
            #[repr(i16)]
            pub enum TestEnum {
                /// Uses the implicit discriminant.
                Default,
                Negative = -2,
                Positive = 3,
            }
        };

        let lowered = lower_enum(&db, db.rustdoc_id("TestEnum")).unwrap();
        let LoweredEnum::CLikeEnum(enum_) = lowered else {
            panic!("unit-only enum must lower to CLikeEnum");
        };

        assert_eq!(
            enum_
                .variants(&db)
                .iter()
                .map(|variant| variant.name(&db).as_str())
                .collect::<Vec<_>>(),
            ["Default", "Negative", "Positive"]
        );
        assert_eq!(
            enum_
                .variants(&db)
                .iter()
                .map(|variant| variant.discriminant(&db).as_deref())
                .collect::<Vec<_>>(),
            [None, Some("-2"), Some("3")]
        );
        assert_eq!(
            enum_.variants(&db)[0].docs(&db).as_deref(),
            Some("Uses the implicit discriminant.")
        );
    }

    #[test]
    fn warns_and_ignores_non_unit_variant_discriminant() {
        let db = test_db! {
            #[repr(u8)]
            pub enum TestEnum {
                Unit = 1,
                Data(u8) = 2,
            }
        };
        let id = db.rustdoc_id("TestEnum");

        let lowered = lower_enum(&db, id).unwrap();
        let LoweredEnum::Enum(enum_) = lowered else {
            panic!("data-carrying enum must lower to Enum");
        };
        assert_eq!(enum_.variants(&db).len(), 2);

        let diagnostics = lower_enum::accumulated::<Diag>(&db, id);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostic_code(diagnostics[0].inner.as_ref()),
            "rust_curse::ignored_non_unit_variant_discriminant"
        );
    }
}
