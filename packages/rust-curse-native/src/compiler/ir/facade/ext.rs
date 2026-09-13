use crate::compiler::{
    db::Db,
    ir::{
        facade::{
            Facade, FacadeClass, FacadeClassDiscriminated, FacadeClassPlain,
            FacadeFieldDiscriminator, FacadeFreeFunction, FacadeMethod, FacadeModule,
            FacadeParentModule,
        },
        shim::{
            Shim, ShimClass, ShimClassDiscriminated, ShimClassPlain, ShimFunctionAssociated,
            ShimFunctionDiscriminator, ShimFunctionFree, ShimModule, ShimParentModule,
        },
    },
};

impl<'db> Shim<'db> {
    pub fn facade(self, db: &'db dyn Db) -> Facade<'db> {
        Facade::new(db, self)
    }
}

impl<'db> ShimParentModule<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        FacadeParentModule::new(db, self)
    }
}

impl<'db> ShimModule<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeModule<'db> {
        FacadeModule::new(db, self)
    }
}

impl<'db> ShimClassPlain<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeClassPlain<'db> {
        FacadeClassPlain::new(db, self)
    }
}

impl<'db> ShimClassDiscriminated<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeClassDiscriminated<'db> {
        FacadeClassDiscriminated::new(db, self)
    }
}

impl<'db> ShimClass<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeClass<'db> {
        match self {
            ShimClass::Plain(plain) => FacadeClass::Plain(plain.facade(db)),
            ShimClass::Discriminated(discriminated) => {
                FacadeClass::Discriminated(discriminated.facade(db))
            }
        }
    }
}

impl<'db> ShimFunctionFree<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeFreeFunction<'db> {
        FacadeFreeFunction::new(db, self)
    }
}

impl<'db> ShimFunctionAssociated<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeMethod<'db> {
        FacadeMethod::new(db, self)
    }
}

impl<'db> ShimFunctionDiscriminator<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeFieldDiscriminator<'db> {
        FacadeFieldDiscriminator::new(db, self)
    }
}
