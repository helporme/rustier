use miette::{Diagnostic, SourceSpan};
use ra_ap_hir::{Adt, GenericDef, GenericParam as HirGenericParam, HasAttrs as _};
use thiserror::Error;

use super::{
    UnstableItemError, adt_fqn, check_stability, diagnostic_location, lower_item_span, lower_shape,
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
    let cause = check_stability(db, hir, hir.module(db), || lower_item_span(db, hir))
        .err()
        .map(StructLowerCause::Unstable)
        .or_else(|| {
            GenericDef::Adt(Adt::Struct(hir))
                .params(db)
                .into_iter()
                .find_map(|param| match param {
                    HirGenericParam::ConstParam(_) => {
                        Some(StructLowerCause::UnsupportedConstGeneric)
                    }
                    HirGenericParam::TypeParam(param)
                        if param.is_unstable(db) && param.default(db).is_none() =>
                    {
                        Some(StructLowerCause::UnsupportedUnstableGeneric)
                    }
                    _ => None,
                })
        });

    if let Some(cause) = cause {
        let span = lower_item_span(db, hir);
        let (source_code, span) = diagnostic_location(db, span.as_ref());
        return Err(Box::new(StructLowerError {
            fqn: adt_fqn(db, Adt::Struct(hir)),
            source_code,
            span,
            causes: vec![cause],
        }));
    }

    Ok(Struct::new(db, hir))
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("the struct \"{fqn}\" cannot be exported to JS")]
#[diagnostic(code(rust_curse::skipped_struct), severity(Warning))]
pub(super) struct StructLowerError {
    fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<SourceSpan>,
    #[related]
    causes: Vec<StructLowerCause>,
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
enum StructLowerCause {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Unstable(#[from] UnstableItemError),

    #[error("const-generic structs are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_const_generic_struct),
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
