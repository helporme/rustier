use std::collections::HashSet;

use ra_ap_hir::{AssocItemContainer, HasSource as _, Semantics};
use ra_ap_syntax::AstNode as _;

use super::lower_impl;
use crate::compiler::{
    db::Db,
    ir::rust::{GenArg, ImplInherent, ImplQualified, ImplTrait, Trait, Type},
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
            .filter_map(|declaration| {
                let hir = declaration.hir(db);
                let Some(trait_) = hir.trait_(db) else {
                    return applicable.contains(&hir).then(|| {
                        ImplQualified::Inherent(ImplInherent::new(db, declaration, self))
                    });
                };
                let impl_ = ImplTrait::new(db, declaration, self, Trait::new(db, trait_));
                let qualified = ImplQualified::Trait(impl_);

                if hir.source(db).is_some() {
                    let candidate = super::instantiate_type(
                        db,
                        hir.self_ty(db),
                        qualified.partial_type_args(db),
                        self.origin(db).context(db),
                    );

                    if candidate.contains_unknown()
                        || !candidate.generic_params(db).is_empty()
                        || !candidate.could_unify_with(db, &target)
                    {
                        return None;
                    }
                }

                let args: Vec<_> = impl_
                    .trait_args(db)
                    .iter()
                    .map(|arg| {
                        let GenArg::Type(type_) = arg;

                        type_.hir(db)
                    })
                    .collect();

                if args.iter().any(|arg| {
                    arg.contains_unknown()
                        || !arg.generic_params(db).is_empty()
                        || super::types::check_type(db, arg, hir.module(db).krate(db)).is_err()
                }) {
                    return None;
                }

                target.impls_trait(db, trait_, &args).then_some(qualified)
            })
            .collect()
    }
}
