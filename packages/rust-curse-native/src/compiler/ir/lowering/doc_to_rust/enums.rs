use super::{ItemExt, RustdocId, RustdocSpan, fields::FieldLowerError, shapes::RustdocShape};
use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource, DiagSpan},
    ir::rust::{Enum, FieldParent, Mod, Shape, Variant},
};
use miette::Diagnostic;
use rustdoc_types as doc;
use thiserror::Error;

#[salsa::tracked]
impl<'db> Enum<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir) fn lower(
        db: &'db dyn Db,
        id: RustdocId<'db>,
        parent: Mod<'db>,
    ) -> Result<Self<'db>, Box<EnumLowerError>> {
        let item = id.item(db);
        let summary = id.summary(db);

        let doc::ItemEnum::Enum(enum_) = &item.inner else {
            panic!("expected enum, got {:?}", item.inner);
        };

        let mut causes = Vec::new();
        if !enum_.generics.params.is_empty() {
            causes.push(EnumLowerCause::UnsupportedGeneric);
        }

        if enum_.has_stripped_variants {
            causes.push(EnumLowerCause::StrippedVariants);
        }

        if !causes.is_empty() {
            let span = item.span.as_ref().map(|span| {
                let crate_name = id.crate_name(db);
                RustdocSpan::new(db, span, crate_name).lower(db)
            });

            let source_code = span.as_ref().map(|span| span.diag_source(db)).into();
            let span = span.as_ref().map(|span| span.diag_span(db));

            return Err(Box::new(EnumLowerError::Skipped {
                enum_fqn: summary.path.join("::"),
                source_code,
                span,
                causes,
            }));
        }

        let name = item.name.as_ref().unwrap();
        Ok(Self::new(db, parent, name))
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir) fn lower_variants(self, db: &'db dyn Db) -> Vec<Variant<'db>> {
        let id = self.rustdoc_id(db);
        let item = id.item(db);

        let doc::ItemEnum::Enum(enum_def) = &item.inner else {
            panic!("expected enum, got {:?}", item.inner);
        };

        let variants = enum_def
            .variants
            .iter()
            .map(|variant_id| {
                let variant_id = RustdocId::new(db, id.crate_name(db), variant_id);
                let variant_item = variant_id.item(db);
                Variant::new(db, self, variant_item.name.as_ref().unwrap().to_owned())
            })
            .collect::<Vec<_>>();

        variants
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir) fn lower_is_non_exhaustive(self, db: &'db dyn Db) -> bool {
        let id = self.rustdoc_id(db);
        let item = id.item(db);
        item.attrs.contains(&doc::Attribute::NonExhaustive)
    }
}

#[salsa::tracked]
impl<'db> Variant<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir) fn lower_shape(self, db: &'db dyn Db) -> Shape<'db> {
        let variant_id = self.rustdoc_id(db);
        let item = variant_id.item(db);

        let doc::ItemEnum::Variant(def) = &item.inner else {
            unreachable!("expected module, got {:?}", item.inner);
        };

        let shape = match &def.kind {
            doc::VariantKind::Plain => RustdocShape::Unit,
            doc::VariantKind::Tuple(ids) => RustdocShape::Tuple(ids),
            doc::VariantKind::Struct {
                fields,
                has_stripped_fields,
            } => RustdocShape::Plain {
                fields,
                has_stripped_fields: *has_stripped_fields,
            },
        };

        let (shape, errors) = Shape::lower(db, shape, FieldParent::Variant(self));

        if !errors.is_empty() {
            let span = self.lower_span(db).as_ref().map(|span| span.diag_span(db));
            let source_code = self
                .lower_span(db)
                .as_ref()
                .map(|span| span.diag_source(db))
                .into();

            VariantLowerError::FieldsSkipped {
                variant_fqn: self.path(db).fqn(db).to_owned(),
                source_code,
                span,
                causes: errors,
            }
            .accumulate(db);
        }

        shape
    }
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
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum VariantLowerError {
    #[error("the enum variant \"{variant_fqn}\" has fields that cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_enum_variant), severity(Warning))]
    FieldsSkipped {
        variant_fqn: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
        #[related]
        causes: Vec<FieldLowerError>,
    },
}
