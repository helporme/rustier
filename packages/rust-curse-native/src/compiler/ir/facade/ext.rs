use crate::compiler::{
    db::Db,
    ir::{
        facade::{FacadeClass, FacadeFieldDiscriminator, FacadeFunction, FacadeMod},
        shim::{
            ShimClass, ShimFunctionAssociated, ShimFunctionEnumDiscriminator, ShimFunctionFree,
            ShimMod,
        },
    },
};

impl<'db> ShimMod<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeMod<'db> {
        FacadeMod::new(db, self)
    }
}

impl<'db> ShimClass<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeClass<'db> {
        FacadeClass::new(db, self)
    }
}

impl<'db> ShimFunctionFree<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeFunction<'db> {
        FacadeFunction::new(db, self.as_fn())
    }
}

impl<'db> ShimFunctionAssociated<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeFunction<'db> {
        FacadeFunction::new(db, self.as_fn())
    }
}

impl<'db> ShimFunctionEnumDiscriminator<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeFieldDiscriminator<'db> {
        FacadeFieldDiscriminator::new(db, self)
    }
}
