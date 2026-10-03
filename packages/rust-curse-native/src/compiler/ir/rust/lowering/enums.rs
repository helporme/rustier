use miette::{Diagnostic, SourceSpan};
use ra_ap_hir::{Adt, GenericDef, GenericParam as HirGenericParam, HasAttrs as _};
use thiserror::Error;

use super::{adt_fqn, diagnostic_location, lower_item_span, lower_shape};
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
    if let Some(cause) = GenericDef::Adt(Adt::Enum(hir))
        .params(db)
        .into_iter()
        .find_map(|param| match param {
            HirGenericParam::ConstParam(_) => Some(EnumLowerCause::UnsupportedConstGeneric),
            HirGenericParam::TypeParam(param)
                if param.is_unstable(db) && param.default(db).is_none() =>
            {
                Some(EnumLowerCause::UnsupportedUnstableGeneric)
            }
            _ => None,
        })
    {
        let span = lower_item_span(db, hir);
        let (source_code, span) = diagnostic_location(db, span.as_ref());
        return Err(Box::new(EnumLowerError {
            fqn: adt_fqn(db, Adt::Enum(hir)),
            source_code,
            span,
            causes: vec![cause],
        }));
    }

    Ok(Enum::new(db, hir))
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("the enum \"{fqn}\" cannot be exported to JS")]
#[diagnostic(code(rust_curse::skipped_enum), severity(Warning))]
pub(super) struct EnumLowerError {
    fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<SourceSpan>,
    #[related]
    causes: Vec<EnumLowerCause>,
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
enum EnumLowerCause {
    #[error("const-generic enums are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_const_generic_enum),
        help("use a type without const generic parameters in the exported API"),
        severity(Warning)
    )]
    UnsupportedConstGeneric,

    #[error("unstable generic parameters without default types are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_unstable_generic),
        help("specify a default type for the #[unstable] generic parameter or remove #[unstable]"),
        severity(Warning)
    )]
    UnsupportedUnstableGeneric,
}
