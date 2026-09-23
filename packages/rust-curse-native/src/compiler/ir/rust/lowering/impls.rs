use ra_ap_hir::{AssocItem, GenericDef, HasSource as _, HasVisibility as _, Visibility};

use super::{lower_function, lower_item_span};
use crate::compiler::{
    db::Db,
    diag::DiagAccumulateExt as _,
    ir::rust::{Impl, Module, Span},
};

#[salsa::tracked]
impl<'db> Impl<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_module(self, db: &'db dyn Db) -> Module<'db> {
        Module::new(db, self.hir(db).module(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_functions(
        self,
        db: &'db dyn Db,
    ) -> Vec<crate::compiler::ir::rust::Function<'db>> {
        self.hir(db)
            .items(db)
            .into_iter()
            .filter_map(|item| match item {
                AssocItem::Function(function) if function.visibility(db) == Visibility::Public => {
                    Some(function)
                }
                _ => None,
            })
            .filter_map(|function| {
                lower_function(db, function)
                    .map_err(|error| (*error).accumulate(db))
                    .ok()
            })
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_span(self, db: &'db dyn Db) -> Option<Span> {
        lower_item_span(db, self.hir(db))
    }
}

pub(super) fn lower_impl<'db>(db: &'db dyn Db, hir: ra_ap_hir::Impl) -> Option<Impl<'db>> {
    if hir.trait_(db).is_some()
        || !GenericDef::Impl(hir).params(db).is_empty()
        || hir.is_unsafe(db)
        || hir.is_negative(db)
        || hir.source(db).is_none()
    {
        return None;
    }

    Some(Impl::new(db, hir))
}
