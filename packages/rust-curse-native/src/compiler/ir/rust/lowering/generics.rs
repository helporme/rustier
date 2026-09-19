use ra_ap_hir::{Adt, GenericDef, GenericParam as HirGenericParam, HirDisplay as _};

use crate::compiler::{
    db::Db,
    ir::rust::{
        Enum, EnumType, Function, GenArg, GenericOwner, GenericParam, Impl, LifetimeParam, Struct,
        StructType, Type, TypeParam,
    },
};

#[salsa::tracked]
impl<'db> TypeParam<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_display(self, db: &'db dyn Db) -> String {
        if let Some(name) = self.name(db) {
            return name.to_owned();
        }
        let hir = self.hir(db);
        let crate_ = hir.module(db).krate(db);
        hir.ty(db)
            .display(db, crate_.to_display_target(db))
            .to_string()
    }
}

#[salsa::tracked]
impl<'db> Function<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_generic_params(
        self,
        db: &'db dyn Db,
    ) -> Vec<GenericParam<'db>> {
        lower_generic_params(db, GenericDef::Function(self.hir(db)))
    }
}

#[salsa::tracked]
impl<'db> Struct<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_generic_params(
        self,
        db: &'db dyn Db,
    ) -> Vec<GenericParam<'db>> {
        lower_generic_params(db, GenericDef::Adt(Adt::Struct(self.hir(db))))
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_as_mono_type(
        self,
        db: &'db dyn Db,
    ) -> Option<Type<'db>> {
        // Check HIR, including unsupported const parameters, not the IR subset.
        GenericDef::Adt(Adt::Struct(self.hir(db)))
            .params(db)
            .is_empty()
            .then(|| StructType::new(db, self, Vec::<GenArg<'db>>::new()).into())
    }
}

#[salsa::tracked]
impl<'db> Enum<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_generic_params(
        self,
        db: &'db dyn Db,
    ) -> Vec<GenericParam<'db>> {
        lower_generic_params(db, GenericDef::Adt(Adt::Enum(self.hir(db))))
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_as_mono_type(
        self,
        db: &'db dyn Db,
    ) -> Option<Type<'db>> {
        GenericDef::Adt(Adt::Enum(self.hir(db)))
            .params(db)
            .is_empty()
            .then(|| EnumType::new(db, self, Vec::<GenArg<'db>>::new()).into())
    }
}

#[salsa::tracked]
impl<'db> Impl<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_generic_params(
        self,
        db: &'db dyn Db,
    ) -> Vec<GenericParam<'db>> {
        lower_generic_params(db, GenericDef::Impl(self.hir(db)))
    }
}

fn lower_generic_params<'db>(db: &'db dyn Db, hir: GenericDef) -> Vec<GenericParam<'db>> {
    hir.params(db)
        .into_iter()
        .filter_map(|param| match param {
            HirGenericParam::TypeParam(hir) => Some(GenericParam::Type(TypeParam::new(db, hir))),
            // Const-generic declarations remain rejected by item/type lowering.
            HirGenericParam::ConstParam(_) => None,
            HirGenericParam::LifetimeParam(hir) => {
                Some(GenericParam::Lifetime(LifetimeParam::new(db, hir)))
            }
        })
        .collect()
}

#[salsa::tracked]
impl<'db> GenericParam<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_name(self, db: &'db dyn Db) -> Option<String> {
        let hir = self.hir(db);
        if let HirGenericParam::TypeParam(param) = hir
            && param.is_implicit(db)
        {
            return None;
        }
        Some(
            hir.name(db)
                .display(db, hir.module(db).krate(db).edition(db))
                .to_string(),
        )
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_owner(self, db: &'db dyn Db) -> GenericOwner<'db> {
        match self.hir(db).parent() {
            GenericDef::Function(hir) => GenericOwner::Function(Function::new(db, hir)),
            GenericDef::Adt(Adt::Struct(hir)) => GenericOwner::Struct(Struct::new(db, hir)),
            GenericDef::Adt(Adt::Enum(hir)) => GenericOwner::Enum(Enum::new(db, hir)),
            GenericDef::Impl(hir) => GenericOwner::Impl(Impl::new(db, hir)),
            _ => unreachable!("only parameters of supported generic owners enter Rust IR"),
        }
    }
}
