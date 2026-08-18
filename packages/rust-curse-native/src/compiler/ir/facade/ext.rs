use crate::compiler::{
    db::Db,
    ir::{
        facade::{FacadeClass, FacadeFunction, FacadeMod, FacadeType},
        shim::{ShimClass, ShimFunctionFree, ShimMod, ShimType},
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
        FacadeFunction::new(db, self)
    }
}

impl<'db> ShimType<'db> {
    pub(super) fn lower_facade(self, db: &'db dyn Db) -> FacadeType<'db> {
        match self {
            ShimType::f64 => FacadeType::number,
            ShimType::String => FacadeType::string,
            ShimType::bool => FacadeType::boolean,
            ShimType::Class { inner, .. } => FacadeType::Class(inner.facade(db)),
        }
    }
}
