use miette::{Diagnostic, SourceSpan};
use ra_ap_hir::{Adt, GenericDef, GenericParam as HirGenericParam, HasAttrs as _};
use thiserror::Error;

use super::{
    adt_fqn, diagnostic_location, instantiate_adt_type, is_stable, lower_item_span, lower_shape,
};
use crate::compiler::{
    db::Db,
    diag::DiagSource,
    ir::rust::{FieldParent, Module, Shape, Span, Struct, StructType},
};

#[salsa::tracked]
impl<'db> StructType<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_has_stripped_fields(self, db: &'db dyn Db) -> bool {
        self.fields(db).count() != self.def(db).hir(db).fields(db).len()
    }
}

#[salsa::tracked]
impl<'db> Struct<'db> {
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
    pub(in crate::compiler::ir::rust) fn lower_shape(self, db: &'db dyn Db) -> Shape<'db> {
        lower_shape(
            db,
            FieldParent::Struct(self),
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

pub(super) fn lower_struct<'db>(
    db: &'db dyn Db,
    hir: ra_ap_hir::Struct,
) -> Result<Struct<'db>, Box<StructLowerError>> {
    let params = GenericDef::Adt(Adt::Struct(hir)).params(db);

    if !is_stable(db, hir, hir.module(db))
        || params.iter().any(|param| {
            matches!(param, HirGenericParam::TypeParam(param)
                if param.is_unstable(db) && param.default(db).is_none())
        })
    {
        return Err(Box::new(StructLowerError::Unstable));
    }

    let mut causes = Vec::new();

    if params
        .iter()
        .any(|param| matches!(param, HirGenericParam::ConstParam(_)))
    {
        causes.push(StructLowerCause::UnsupportedConstGeneric);
    }

    let type_ = instantiate_adt_type(
        db,
        hir.ty(db),
        Adt::Struct(hir),
        std::iter::empty(),
        GenericDef::Adt(Adt::Struct(hir)),
    );

    if super::types::contains_borrowed_values(db, &type_) {
        causes.push(StructLowerCause::BorrowedValues);
    }

    if !causes.is_empty() {
        let span = lower_item_span(db, hir);
        let (source_code, span) = diagnostic_location(db, span.as_ref());

        return Err(Box::new(StructLowerError::Unsupported {
            fqn: adt_fqn(db, Adt::Struct(hir)),
            source_code,
            span,
            causes,
        }));
    }

    Ok(Struct::new(db, hir))
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub(super) enum StructLowerError {
    #[error("unstable items are not supported")]
    Unstable,

    #[error("the struct \"{fqn}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_struct), severity(Warning))]
    Unsupported {
        fqn: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<SourceSpan>,
        #[related]
        causes: Vec<StructLowerCause>,
    },
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub(super) enum StructLowerCause {
    #[error("const-generic structs are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_const_generic_struct),
        help("use a type without const generic parameters in the exported API"),
        severity(Warning)
    )]
    UnsupportedConstGeneric,
    #[error("structs containing borrowed values are not supported")]
    #[diagnostic(
        code(rust_curse::borrowed_struct),
        help("use owned values inside the struct"),
        severity(Warning)
    )]
    BorrowedValues,
}
