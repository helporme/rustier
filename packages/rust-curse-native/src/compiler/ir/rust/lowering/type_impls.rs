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
        ra_ap_hir::Impl::all_for_type(db, target.clone())
            .into_iter()
            .filter_map(|impl_| lower_impl(db, impl_))
            .filter(|impl_| {
                impl_
                    .hir(db)
                    .self_ty(db)
                    .try_rebase_into(db, &target)
                    .is_some_and(|self_ty| self_ty.could_unify_with_deeply(db, &target))
            })
            .map(|impl_| ImplQualified::Inherent(ImplInherent::new(db, impl_, self)))
            .collect()
    }
}
