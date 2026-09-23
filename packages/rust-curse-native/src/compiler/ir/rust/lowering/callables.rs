use ra_ap_hir::GenericDef;

use crate::compiler::{
    db::Db,
    ir::rust::{
        Callable, Crate, FunctionInherent, FunctionQualified, GenArg, ItemPath, Module, Param,
        PathSegment, Type, TypeKind, TypeOrigin,
    },
};

#[salsa::tracked]
impl<'db> FunctionQualified<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_as_callible(
        self,
        db: &'db dyn Db,
    ) -> Option<Callable<'db>> {
        GenericDef::Function(self.declaration(db).hir(db))
            .params(db)
            .is_empty()
            .then(|| Callable::new(db, self, Vec::<GenArg<'db>>::new()))
    }
}

#[salsa::tracked]
impl<'db> FunctionInherent<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_path(self, db: &'db dyn Db) -> ItemPath<'db> {
        let target = self.target(db);
        let path = match target.kind(db) {
            TypeKind::Struct(type_) => type_.path(db),
            TypeKind::Enum(type_) => type_.path(db),
            _ => {
                let hir = target.hir(db);
                if let Some(adt) = hir.as_adt() {
                    let module = adt.module(db);
                    let name = adt
                        .name(db)
                        .display(db, module.krate(db).edition(db))
                        .to_string();
                    Module::new(db, module)
                        .path(db)
                        .append(db, PathSegment::plain(db, name))
                } else {
                    let crate_ = self.impl_(db).declaration(db).hir(db).module(db).krate(db);
                    ItemPath::from_segment(
                        db,
                        Crate::new(db, crate_),
                        PathSegment::plain(db, target.display(db).to_owned()),
                    )
                }
            }
        };
        path.append(db, self.segment(db))
    }
}

#[salsa::tracked]
impl<'db> Callable<'db> {
    pub(in crate::compiler::ir::rust) fn needs_substitution(self, db: &'db dyn Db) -> bool {
        !GenericDef::Function(self.declaration(db).hir(db))
            .params(db)
            .is_empty()
            || match self.function(db) {
                FunctionQualified::Free(_) => false,
                FunctionQualified::Inherent(function) => {
                    !GenericDef::Impl(function.impl_(db).declaration(db).hir(db))
                        .params(db)
                        .is_empty()
                }
            }
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_params(self, db: &'db dyn Db) -> Vec<Param<'db>> {
        if !self.needs_substitution(db) {
            return self.declaration(db).params(db).clone();
        }

        self.declaration(db)
            .params(db)
            .iter()
            .map(|param| {
                Param::new(
                    db,
                    param.index(db),
                    param.name(db).clone(),
                    Type::new(
                        db,
                        TypeOrigin::CallableParameter {
                            callable: self,
                            index: param.index(db),
                        },
                    ),
                    param.span(db).clone(),
                )
            })
            .collect()
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_ret_type(
        self,
        db: &'db dyn Db,
    ) -> Option<Type<'db>> {
        if !self.needs_substitution(db) {
            return self.declaration(db).ret_type(db);
        }
        let type_ = Type::new(db, TypeOrigin::CallableReturn(self));
        (!type_.hir(db).is_unit()).then_some(type_)
    }

    pub(super) fn instantiate(
        self,
        db: &'db dyn Db,
        type_: ra_ap_hir::Type<'db>,
    ) -> ra_ap_hir::Type<'db> {
        let mut args = Vec::new();
        if let FunctionQualified::Inherent(function) = self.function(db) {
            let impl_ = function.impl_(db).declaration(db).hir(db);
            let mut bindings = std::collections::HashMap::new();

            bind_self_type(
                db,
                impl_.self_ty(db),
                function.target(db).hir(db),
                &mut bindings,
            );

            for param in GenericDef::Impl(impl_).params(db) {
                if let ra_ap_hir::GenericParam::TypeParam(param) = param {
                    args.push(
                        bindings
                            .remove(&param)
                            .expect("impl type parameter must be determined by its target"),
                    );
                }
            }
        }

        args.extend(self.generic_args(db).iter().filter_map(|arg| match arg {
            GenArg::Type(type_) => Some(type_.hir(db)),
            GenArg::Lifetime(_) => None,
        }));

        if args.is_empty() {
            type_
        } else {
            type_.instantiate(args)
        }
    }
}

fn bind_self_type<'db>(
    db: &'db dyn Db,
    pattern: ra_ap_hir::Type<'db>,
    target: ra_ap_hir::Type<'db>,
    bindings: &mut std::collections::HashMap<ra_ap_hir::TypeParam, ra_ap_hir::Type<'db>>,
) {
    if let Some(param) = pattern.as_type_param(db) {
        bindings.insert(param, target);
        return;
    }
    if let Some((inner, _)) = pattern.as_reference() {
        let (target, _) = target.as_reference().expect("impl target reference");
        bind_self_type(db, inner, target, bindings);
        return;
    }
    if pattern.as_adt().is_some() {
        for (pattern, target) in pattern.type_arguments().zip(target.type_arguments()) {
            bind_self_type(db, pattern, target, bindings);
        }
    }
}
