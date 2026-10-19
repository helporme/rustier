use ra_ap_hir::{Adt, GenericDef, GenericParam as HirGenericParam, HirDisplay as _};

use crate::compiler::{
    db::Db,
    ir::rust::{
        Enum, Function, GenericOwner, GenericParam, Impl, Struct, Trait, Type, TypeOrigin,
        TypeParam,
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
        self.generic_params(db)
            .is_empty()
            .then(|| Type::new(db, TypeOrigin::StructDeclaration(self)))
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
        self.generic_params(db)
            .is_empty()
            .then(|| Type::new(db, TypeOrigin::EnumDeclaration(self)))
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

#[salsa::tracked]
impl<'db> Trait<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_generic_params(
        self,
        db: &'db dyn Db,
    ) -> Vec<GenericParam<'db>> {
        lower_generic_params(db, GenericDef::Trait(self.hir(db)))
            .into_iter()
            .filter(|param| param.name(db).is_some())
            .collect()
    }
}

fn lower_generic_params<'db>(db: &'db dyn Db, hir: GenericDef) -> Vec<GenericParam<'db>> {
    hir.params(db)
        .into_iter()
        .filter_map(|param| match param {
            HirGenericParam::TypeParam(hir)
                if !hir.is_unstable(db) || hir.default(db).is_none() =>
            {
                Some(GenericParam::Type(TypeParam::new(db, hir)))
            }
            _ => None,
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
            GenericDef::Trait(hir) => GenericOwner::Trait(Trait::new(db, hir)),
            _ => unreachable!("only parameters of supported generic owners enter Rust IR"),
        }
    }
}
