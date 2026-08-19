use crate::compiler::{
    db::Db,
    ir::{
        decals::{DecalClass, DecalFunction, DecalMod},
        shim::{ShimClass, ShimFunctionFree, ShimMod},
    },
};

impl<'db> ShimMod<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalMod<'db> {
        DecalMod::new(db, self)
    }
}

impl<'db> ShimClass<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalClass<'db> {
        DecalClass::new(db, self)
    }
}

impl<'db> ShimFunctionFree<'db> {
    pub fn decals(self, db: &'db dyn Db) -> DecalFunction<'db> {
        DecalFunction::new(db, self)
    }
}
