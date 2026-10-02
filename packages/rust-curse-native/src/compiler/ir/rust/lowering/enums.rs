use miette::{Diagnostic, SourceSpan};
use ra_ap_hir::{Adt, GenericDef, HasAttrs as _};
use thiserror::Error;

use super::{adt_fqn, diagnostic_location, lower_item_span, lower_shape};
use crate::compiler::{
    db::Db,
    diag::DiagSource,
    ir::rust::{Enum, EnumType, FieldParent, Module, Shape, Span, Variant, VariantQualified},
};

#[salsa::tracked]
impl<'db> EnumType<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_is_clonable(self, db: &'db dyn Db) -> bool {
        let crate_ = self.def(db).hir(db).module(db).krate(db);
        super::is_clonable(db, &self.into_type(db).hir(db), crate_)
    }
}

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
    let mut causes = Vec::new();
    if !GenericDef::Adt(Adt::Enum(hir)).params(db).is_empty() {
        causes.push(EnumLowerCause::UnsupportedGeneric);
    }
    if causes.is_empty() {
        return Ok(Enum::new(db, hir));
    }

    let span = lower_item_span(db, hir);
    let (source_code, span) = diagnostic_location(db, span.as_ref());
    Err(Box::new(EnumLowerError {
        fqn: adt_fqn(db, Adt::Enum(hir)),
        source_code,
        span,
        causes,
    }))
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
    #[error("generic enums are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_generic_enum),
        help(
            "export a non-generic wrapper that instantiates the generic parameters with concrete types"
        ),
        severity(Warning)
    )]
    UnsupportedGeneric,
}
