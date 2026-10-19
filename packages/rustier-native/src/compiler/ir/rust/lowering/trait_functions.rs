use ra_ap_hir::AssocItem;

use super::functions::{FunctionLowerError, check_function_substituted, lower_function};
use crate::compiler::{
    db::Db,
    diag::DiagAccumulateExt as _,
    ir::rust::{Callable, FunctionQualified, FunctionTrait, GenArg, ImplQualified, ImplTrait},
};

#[salsa::tracked]
impl<'db> ImplTrait<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_functions(
        self,
        db: &'db dyn Db,
    ) -> Vec<FunctionTrait<'db>> {
        let trait_ = self.trait_(db).hir(db);
        let crate_ = trait_.module(db).krate(db);

        if Some(trait_) == ra_ap_hir::Trait::lang(db, crate_, ra_ap_hir::LangItem::Drop) {
            return Vec::new();
        }

        let args = ImplQualified::Trait(self).function_type_args(db);
        let context = self.target(db).origin(db).context(db);

        trait_
            .items(db)
            .into_iter()
            .filter_map(|item| {
                let AssocItem::Function(hir) = item else {
                    return None;
                };

                let declaration = lower_function(db, hir)
                    .map_err(|error| {
                        if matches!(*error, FunctionLowerError::Unsupported { .. }) {
                            (*error).accumulate(db);
                        }
                    })
                    .ok()?;

                if !declaration.generic_params(db).is_empty() {
                    return None;
                }

                if let Some(error) = check_function_substituted(db, hir, &args, context, Vec::new())
                {
                    error.accumulate(db);

                    return None;
                }

                Some(FunctionTrait::new(db, declaration, self))
            })
            .collect()
    }
}

#[salsa::tracked]
impl<'db> FunctionTrait<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_as_callable(
        self,
        db: &'db dyn Db,
    ) -> Option<Callable<'db>> {
        let callable = Callable::new(
            db,
            FunctionQualified::Trait(self),
            Vec::<GenArg<'db>>::new(),
        );

        super::callables::evaluate_where_clause(db, callable)?.then_some(callable)
    }
}
