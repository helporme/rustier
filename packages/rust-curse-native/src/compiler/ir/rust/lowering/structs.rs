use miette::{Diagnostic, SourceSpan};
use ra_ap_hir::{Adt, GenericDef, HasAttrs as _};
use thiserror::Error;

use super::{adt_fqn, diagnostic_location, lower_item_span, lower_shape};
use crate::compiler::{
    db::Db,
    diag::DiagSource,
    ir::rust::{FieldParent, Mod, Shape, Span, Struct},
};

#[salsa::tracked]
impl<'db> Struct<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_parent(self, db: &'db dyn Db) -> Mod<'db> {
        Mod::new(db, self.hir(db).module(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_name(self, db: &'db dyn Db) -> String {
        let hir = self.hir(db);
        hir.name(db)
            .display(db, hir.module(db).krate(db).edition(db))
            .to_string()
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
    if GenericDef::Adt(Adt::Struct(hir)).params(db).is_empty() {
        return Ok(Struct::new(db, hir));
    }

    let span = lower_item_span(db, hir);
    let (source_code, span) = diagnostic_location(db, span.as_ref());
    Err(Box::new(StructLowerError {
        fqn: adt_fqn(db, Adt::Struct(hir)),
        source_code,
        span,
        causes: vec![StructLowerCause::UnsupportedGeneric],
    }))
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
    #[error("generic structs are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_generic_struct),
        help(
            "export a non-generic wrapper that instantiates the generic parameters with concrete types"
        ),
        severity(Warning)
    )]
    UnsupportedGeneric,
}
