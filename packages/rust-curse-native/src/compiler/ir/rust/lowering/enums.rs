use miette::{Diagnostic, SourceSpan};
use ra_ap_hir::{Adt, GenericDef, GenericParam as HirGenericParam, HasAttrs as _};
use thiserror::Error;

use super::{
    adt_fqn, diagnostic_location, instantiate_adt_type, is_stable, lower_item_span, lower_shape,
};
use crate::compiler::{
    db::Db,
    diag::DiagSource,
    ir::rust::{Enum, FieldParent, Module, Shape, Span, Variant, VariantQualified},
};

#[salsa::tracked]
impl<'db> VariantQualified<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_has_stripped_fields(self, db: &'db dyn Db) -> bool {
        self.fields(db).count() != self.declaration(db).hir(db).fields(db).len()
    }
}

#[salsa::tracked]
impl<'db> Enum<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_parent(self, db: &'db dyn Db) -> Module<'db> {
        Module::new(db, self.hir(db).module(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_name(self, db: &'db dyn Db) -> String {
        let hir = self.hir(db);
        hir.name(db)
            .display(db, hir.module(db).krate(db).edition(db))
            .to_string()
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_is_non_exhaustive(self, db: &'db dyn Db) -> bool {
        self.hir(db).attrs(db).is_non_exhaustive()
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_variants(
        self,
        db: &'db dyn Db,
    ) -> Vec<Variant<'db>> {
        self.hir(db)
            .variants(db)
            .into_iter()
            .map(|variant| Variant::new(db, variant))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_docs(self, db: &'db dyn Db) -> Option<String> {
        self.hir(db).hir_docs(db).map(|docs| docs.docs().to_owned())
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_span(self, db: &'db dyn Db) -> Option<Span> {
        lower_item_span(db, self.hir(db))
    }
}

#[salsa::tracked]
impl<'db> Variant<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_parent(self, db: &'db dyn Db) -> Enum<'db> {
        Enum::new(db, self.hir(db).parent_enum(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_name(self, db: &'db dyn Db) -> String {
        let hir = self.hir(db);
        hir.name(db)
            .display(db, hir.module(db).krate(db).edition(db))
            .to_string()
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_is_non_exhaustive(self, db: &'db dyn Db) -> bool {
        self.hir(db).attrs(db).is_non_exhaustive()
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_shape(self, db: &'db dyn Db) -> Shape<'db> {
        lower_shape(
            db,
            FieldParent::Variant(self),
            self.hir(db).kind(db),
            self.hir(db).fields(db),
        )
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_docs(self, db: &'db dyn Db) -> Option<String> {
        self.hir(db).hir_docs(db).map(|docs| docs.docs().to_owned())
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_span(self, db: &'db dyn Db) -> Option<Span> {
        lower_item_span(db, self.hir(db))
    }
}

pub(super) fn lower_enum<'db>(
    db: &'db dyn Db,
    hir: ra_ap_hir::Enum,
) -> Result<Enum<'db>, Box<EnumLowerError>> {
    let params = GenericDef::Adt(Adt::Enum(hir)).params(db);

    if !is_stable(db, hir, hir.module(db))
        || hir
            .variants(db)
            .into_iter()
            .any(|variant| !is_stable(db, variant, hir.module(db)))
        || params.iter().any(|param| {
            matches!(param, HirGenericParam::TypeParam(param)
                if param.is_unstable(db) && param.default(db).is_none())
        })
    {
        return Err(Box::new(EnumLowerError::Unstable));
    }

    let mut causes = Vec::new();

    if params
        .iter()
        .any(|param| matches!(param, HirGenericParam::ConstParam(_)))
    {
        causes.push(EnumLowerCause::UnsupportedConstGeneric);
    }

    let type_ = instantiate_adt_type(
        db,
        hir.ty(db),
        Adt::Enum(hir),
        std::iter::empty(),
        GenericDef::Adt(Adt::Enum(hir)),
    );

    if super::types::contains_borrowed_values(db, &type_) {
        causes.push(EnumLowerCause::BorrowedValues);
    }

    if !causes.is_empty() {
        let span = lower_item_span(db, hir);
        let (source_code, span) = diagnostic_location(db, span.as_ref());

        return Err(Box::new(EnumLowerError::Unsupported {
            fqn: adt_fqn(db, Adt::Enum(hir)),
            source_code,
            span,
            causes,
        }));
    }

    Ok(Enum::new(db, hir))
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub(super) enum EnumLowerError {
    #[error("unstable items are not supported")]
    Unstable,

    #[error("the enum \"{fqn}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_enum), severity(Warning))]
    Unsupported {
        fqn: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<SourceSpan>,
        #[related]
        causes: Vec<EnumLowerCause>,
    },
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub(super) enum EnumLowerCause {
    #[error("const-generic enums are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_const_generic_enum),
        help("use a type without const generic parameters in the exported API"),
        severity(Warning)
    )]
    UnsupportedConstGeneric,
    #[error("enums containing borrowed values are not supported")]
    #[diagnostic(
        code(rust_curse::borrowed_enum),
        help("use owned values inside the enum"),
        severity(Warning)
    )]
    BorrowedValues,
}
