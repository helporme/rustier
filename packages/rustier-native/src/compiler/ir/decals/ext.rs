use crate::compiler::{
    db::Db,
    ir::{
        decals::{
            DecalClass, DecalClassDiscriminated, DecalClassPlain, DecalFreeFunction, DecalMethod,
            DecalModule, DecalParentModule, Decals,
        },
        facade::{
            Facade, FacadeClass, FacadeClassDiscriminated, FacadeClassPlain, FacadeFreeFunction,
            FacadeMethod, FacadeModule, FacadeParentModule,
        },
    },
};

impl<'db> Facade<'db> {
    pub fn decals(self, db: &'db dyn Db) -> Decals<'db> {
        Decals::new(db, self)
    }
}

impl<'db> FacadeParentModule<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalParentModule<'db> {
        DecalParentModule::new(db, self)
    }
}

impl<'db> FacadeModule<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalModule<'db> {
        DecalModule::new(db, self)
    }
}

impl<'db> FacadeClassPlain<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalClassPlain<'db> {
        DecalClassPlain::new(db, self)
    }
}

impl<'db> FacadeClassDiscriminated<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalClassDiscriminated<'db> {
        DecalClassDiscriminated::new(db, self)
    }
}

impl<'db> FacadeClass<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalClass<'db> {
        match self {
            FacadeClass::Plain(plain) => DecalClass::Plain(plain.decals(db)),
            FacadeClass::Discriminated(discriminated) => {
                DecalClass::Discriminated(discriminated.decals(db))
            }
        }
    }
}

impl<'db> FacadeFreeFunction<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalFreeFunction<'db> {
        DecalFreeFunction::new(db, self)
    }
}

impl<'db> FacadeMethod<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalMethod<'db> {
        DecalMethod::new(db, self)
    }
}
