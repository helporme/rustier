use crate::compiler::{
    db::Db,
    ir::{
        decals::{DecalClass, DecalFreeFunction, DecalMethod, DecalModule},
        shim::{ShimClass, ShimFunctionAssociated, ShimFunctionFree, ShimModule},
    },
};

impl<'db> ShimModule<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalModule<'db> {
        DecalModule::new(db, self)
    }
}

impl<'db> ShimClass<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalClass<'db> {
        DecalClass::new(db, self)
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
