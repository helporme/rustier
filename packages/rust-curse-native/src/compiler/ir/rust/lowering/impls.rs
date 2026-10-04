use std::collections::HashMap;

use ra_ap_hir::{
    AssocItem, GenericDef, GenericParam, HasSource as _, HasVisibility as _, Type as HirType,
    TypeParam as HirTypeParam, Visibility,
};

use super::{check_stability, functions::lower_inherent_function, lower_function, lower_item_span};
use crate::compiler::{
    db::Db,
    diag::DiagAccumulateExt as _,
    ir::rust::{
        FunctionInherent, GenArg, GenericParam as RustGenericParam, Impl, ImplInherent, Module,
        Span, Type, TypeOrigin,
    },
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

#[salsa::tracked]
impl<'db> ImplInherent<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_generic_args(
        self,
        db: &'db dyn Db,
    ) -> Vec<(RustGenericParam<'db>, GenArg<'db>)> {
        let mut bindings = HashMap::new();
        bind_self_type(
            db,
            self.declaration(db).hir(db).self_ty(db),
            self.target(db),
            &mut bindings,
        );
        self.declaration(db)
            .generic_params(db)
            .iter()
            .filter_map(|param| {
                let RustGenericParam::Type(type_param) = param;
                bindings
                    .remove(&type_param.hir(db))
                    .map(|type_| (*param, GenArg::Type(type_)))
            })
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_functions(
        self,
        db: &'db dyn Db,
    ) -> Vec<FunctionInherent<'db>> {
        self.declaration(db)
            .hir(db)
            .items(db)
            .into_iter()
            .filter_map(|item| match item {
                AssocItem::Function(function) if function.visibility(db) == Visibility::Public => {
                    lower_inherent_function(db, function, self)
                        .map_err(|error| (*error).accumulate(db))
                        .ok()
                }
                _ => None,
            })
            .collect()
    }
}

pub(super) fn lower_impl<'db>(db: &'db dyn Db, hir: ra_ap_hir::Impl) -> Option<Impl<'db>> {
    if hir.trait_(db).is_some()
        || GenericDef::Impl(hir)
            .params(db)
            .iter()
            .any(|param| matches!(param, GenericParam::ConstParam(_)))
        || hir.is_unsafe(db)
        || hir.is_negative(db)
        || hir.source(db).is_none()
    {
        return None;
    }

    let unstable = check_stability(db, hir, hir.module(db), || lower_item_span(db, hir))
        .err()
        .or_else(|| {
            GenericDef::Impl(hir)
                .params(db)
                .into_iter()
                .find_map(|param| match param {
                    GenericParam::TypeParam(param)
                        if param.is_unstable(db) && param.default(db).is_none() =>
                    {
                        check_stability(db, param, hir.module(db), || lower_item_span(db, hir))
                            .err()
                    }
                    _ => None,
                })
        });

    if let Some(error) = unstable {
        error.accumulate(db);
        return None;
    }

    Some(Impl::new(db, hir))
}

fn bind_self_type<'db>(
    db: &'db dyn Db,
    pattern: HirType<'db>,
    target: Type<'db>,
    bindings: &mut HashMap<HirTypeParam, Type<'db>>,
) {
    let target_hir = target.hir(db);
    if let Some(param) = pattern.as_type_param(db) {
        bindings.insert(param, target);
        return;
    }
    if let Some((inner, _)) = pattern.as_reference() {
        if target_hir.as_reference().is_some() {
            bind_self_type(
                db,
                inner,
                Type::new(db, TypeOrigin::Dereferenced(target)),
                bindings,
            );
        }
        return;
    }
    if pattern.as_adt().is_some() && pattern.as_adt() == target_hir.as_adt() {
        for (index, pattern) in pattern.type_arguments().enumerate() {
            bind_self_type(
                db,
                pattern,
                Type::new(
                    db,
                    TypeOrigin::TypeArgument {
                        type_: target,
                        index,
                    },
                ),
                bindings,
            );
        }
    }
}
