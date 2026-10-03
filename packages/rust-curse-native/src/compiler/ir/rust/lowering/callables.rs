use crate::compiler::{
    db::Db,
    ir::rust::{
        Callable, Crate, FunctionInherent, FunctionQualified, GenArg, GenericParam, ImplInherent,
        ItemPath, Lifetime, Module, Param, PathSegment, Type, TypeKind, TypeOrigin,
    },
};

#[salsa::tracked]
impl<'db> FunctionInherent<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_as_callable(
        self,
        db: &'db dyn Db,
    ) -> Option<Callable<'db>> {
        let impl_ = self.impl_(db);
        let substituted_args = impl_
            .generic_args(db)
            .iter()
            .filter(|(_, arg)| match arg {
                GenArg::Type(type_) => {
                    let hir = type_.hir(db);
                    !hir.contains_unknown() && hir.generic_params(db).is_empty()
                }
                GenArg::Lifetime(Some(Lifetime::Static)) => true,
                GenArg::Lifetime(_) => false,
            })
            .count();

        if !self.declaration(db).generic_params(db).is_empty()
            || substituted_args != impl_.declaration(db).generic_params(db).len()
        {
            return None;
        }

        Some(Callable::new(
            db,
            FunctionQualified::Inherent(self),
            Vec::<GenArg<'db>>::new(),
        ))
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_path(self, db: &'db dyn Db) -> ItemPath<'db> {
        let target = self.impl_(db).target(db);
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
        path.append(db, self.declaration(db).segment(db))
    }
}

#[salsa::tracked]
impl<'db> Callable<'db> {
    pub(in crate::compiler::ir::rust) fn needs_substitution(self, db: &'db dyn Db) -> bool {
        !self.declaration(db).generic_params(db).is_empty()
            || match self.function(db) {
                FunctionQualified::Free(_) => false,
                FunctionQualified::Inherent(function) => !function
                    .impl_(db)
                    .declaration(db)
                    .generic_params(db)
                    .is_empty(),
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
            args.extend(function.impl_(db).partial_type_args(db));
        }

        args.extend(self.generic_args(db).iter().filter_map(|arg| match arg {
            GenArg::Type(type_) => Some(type_.hir(db)),
            GenArg::Lifetime(_) => None,
        }));

        super::instantiate_type(db, type_, args)
    }
}

impl<'db> ImplInherent<'db> {
    /// Preserve identity arguments for bindings not determined by the target.
    pub(super) fn partial_type_args(self, db: &'db dyn Db) -> Vec<ra_ap_hir::Type<'db>> {
        self.declaration(db)
            .generic_params(db)
            .iter()
            .filter_map(|param| match param {
                GenericParam::Type(type_param) => Some(
                    self.generic_args(db)
                        .iter()
                        .find_map(|(binding, arg)| match arg {
                            GenArg::Type(type_) if binding == param => Some(type_.hir(db)),
                            _ => None,
                        })
                        .unwrap_or_else(|| type_param.hir(db).ty(db)),
                ),
                GenericParam::Lifetime(_) => None,
            })
            .collect()
    }
}
