use crate::compiler::{
    db::Db,
    ir::{
        facade::{
            Facade, FacadeClass, FacadeClassDiscriminated, FacadeClassPlain, FacadeFreeFunction,
            FacadeMethod, FacadeModule, FacadeParentModule,
        },
        interface::{
            InterClass, InterClassDiscriminated, InterClassPlain, InterFunctionAssociated,
            InterFunctionFree, InterModule, InterParentModule, Interface,
        },
    },
};

impl<'db> Interface<'db> {
    pub fn facade(self, db: &'db dyn Db) -> Facade<'db> {
        Facade::new(db, self)
    }
}

impl<'db> InterParentModule<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        FacadeParentModule::new(db, self)
    }
}

impl<'db> InterModule<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeModule<'db> {
        FacadeModule::new(db, self)
    }
}

impl<'db> InterClassPlain<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeClassPlain<'db> {
        FacadeClassPlain::new(db, self)
    }
}

impl<'db> InterClassDiscriminated<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeClassDiscriminated<'db> {
        FacadeClassDiscriminated::new(db, self)
    }
}

impl<'db> InterClass<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeClass<'db> {
        match self {
            InterClass::Plain(plain) => FacadeClass::Plain(plain.facade(db)),
            InterClass::Discriminated(discriminated) => {
                FacadeClass::Discriminated(discriminated.facade(db))
            }
        }
    }
}

impl<'db> InterFunctionFree<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeFreeFunction<'db> {
        FacadeFreeFunction::new(db, self)
    }
}

impl<'db> InterFunctionAssociated<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeMethod<'db> {
        FacadeMethod::new(db, self)
    }
}
