use crate::compiler::{
    db::Db,
    ir::{
        decals::{
            DecalClass, DecalClassDiscriminated, DecalClassPlain, DecalFreeFunction, DecalMethod,
            DecalModule, DecalParentModule, Decals,
        },
        shim::{
            Shim, ShimClass, ShimClassDiscriminated, ShimClassPlain, ShimFunctionAssociated,
            ShimFunctionFree, ShimModule, ShimParentModule,
        },
    },
};

impl<'db> Shim<'db> {
    pub fn decals(self, db: &'db dyn Db) -> Decals<'db> {
        Decals::new(db, self)
    }
}

impl<'db> ShimParentModule<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalParentModule<'db> {
        DecalParentModule::new(db, self)
    }
}

impl<'db> ShimModule<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalModule<'db> {
        DecalModule::new(db, self)
    }
}

impl<'db> ShimClassPlain<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalClassPlain<'db> {
        DecalClassPlain::new(db, self)
    }
}

impl<'db> ShimClassDiscriminated<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalClassDiscriminated<'db> {
        DecalClassDiscriminated::new(db, self)
    }
}

impl<'db> ShimClass<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalClass<'db> {
        match self {
            ShimClass::Plain(plain) => DecalClass::Plain(plain.decals(db)),
            ShimClass::Discriminated(discriminated) => {
                DecalClass::Discriminated(discriminated.decals(db))
            }
        }
    }
}

impl<'db> ShimFunctionFree<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalFreeFunction<'db> {
        DecalFreeFunction::new(db, self)
    }
}

impl<'db> ShimFunctionAssociated<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalMethod<'db> {
        DecalMethod::new(db, self)
    }
}
