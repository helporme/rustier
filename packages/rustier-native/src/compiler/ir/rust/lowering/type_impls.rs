use std::collections::HashSet;

use ra_ap_hir::{AssocItemContainer, Semantics};
use ra_ap_syntax::AstNode as _;

use super::lower_impl;
use crate::compiler::{
    db::Db,
    ir::rust::{ImplInherent, ImplQualified, Type},
};

#[salsa::tracked]
impl<'db> Type<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_impls(
        self,
        db: &'db dyn Db,
    ) -> Vec<ImplQualified<'db>> {
        let target = self.hir(db);
        let candidates: Vec<_> = ra_ap_hir::Impl::all_for_type(db, target.clone())
            .into_iter()
            .filter_map(|impl_| lower_impl(db, impl_))
            .collect();

        let sema = Semantics::new_dyn(db);
        let source = target
            .as_adt()
            .and_then(|adt| sema.source(adt))
            .map(|source| source.value.syntax().clone())
            .or_else(|| {
                candidates.iter().find_map(|impl_| {
                    sema.source(impl_.hir(db))
                        .map(|source| source.value.syntax().clone())
                })
            });

        let Some(scope) = source.as_ref().and_then(|source| sema.scope(source)) else {
            return Vec::new();
        };
        let mut applicable = HashSet::new();

        target.iterate_path_candidates(db, &scope, &Default::default(), None, |item| {
            if let AssocItemContainer::Impl(impl_) = item.container(db) {
                applicable.insert(impl_);
            }
            None::<()>
        });

        candidates
            .into_iter()
            .filter(|impl_| applicable.contains(&impl_.hir(db)))
            .map(|impl_| ImplQualified::Inherent(ImplInherent::new(db, impl_, self)))
            .collect()
    }
}
