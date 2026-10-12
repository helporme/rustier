use crate::compiler::{
    db::Db,
    ir::{
        decals::{
            DecalClass, DecalClassDiscriminated, DecalClassPlain, DecalFreeFunction, DecalMethod,
            DecalModule, DecalParentModule, Decals,
        },
        interface::{
            InterClass, InterClassDiscriminated, InterClassPlain, InterFunctionAssociated,
            InterFunctionFree, InterModule, InterParentModule, Interface,
        },
    },
};

impl<'db> Interface<'db> {
    pub fn decals(self, db: &'db dyn Db) -> Decals<'db> {
        Decals::new(db, self)
    }
}

impl<'db> InterParentModule<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalParentModule<'db> {
        DecalParentModule::new(db, self)
    }
}

impl<'db> InterModule<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalModule<'db> {
        DecalModule::new(db, self)
    }
}

impl<'db> InterClassPlain<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalClassPlain<'db> {
        DecalClassPlain::new(db, self)
    }
}

impl<'db> InterClassDiscriminated<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalClassDiscriminated<'db> {
        DecalClassDiscriminated::new(db, self)
    }
}

impl<'db> InterClass<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalClass<'db> {
        match self {
            InterClass::Plain(plain) => DecalClass::Plain(plain.decals(db)),
            InterClass::Discriminated(discriminated) => {
                DecalClass::Discriminated(discriminated.decals(db))
            }
        }
    }
}

impl<'db> InterFunctionFree<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalFreeFunction<'db> {
        DecalFreeFunction::new(db, self)
    }
}

impl<'db> InterFunctionAssociated<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalMethod<'db> {
        DecalMethod::new(db, self)
    }
}
