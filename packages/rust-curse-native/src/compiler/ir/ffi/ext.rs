use crate::compiler::{
    db::Db,
    ir::{
        ffi::{FfiClass, FfiFunction, FfiParam},
        shim::{
            ShimClass, ShimClassDiscriminated, ShimClassPlain, ShimFunction,
            ShimFunctionAssociated, ShimFunctionDiscriminatedCoerce, ShimFunctionDiscriminator,
            ShimFunctionFree, ShimFunctionGetter, ShimFunctionPlainCoerce, ShimFunctionSetter,
            ShimParam,
        },
    },
};

impl<'db> ShimClass<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        FfiClass::new(db, self)
    }
}

impl<'db> ShimClassPlain<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        FfiClass::new(db, ShimClass::Plain(self))
    }
}

impl<'db> ShimClassDiscriminated<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        FfiClass::new(db, ShimClass::Discriminated(self))
    }
}

impl<'db> ShimFunction<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, self)
    }
}

impl<'db> ShimFunctionFree<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, ShimFunction::from(self))
    }
}

impl<'db> ShimFunctionAssociated<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, ShimFunction::from(self))
    }
}

impl<'db> ShimFunctionGetter<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, ShimFunction::from(self))
    }
}

impl<'db> ShimFunctionSetter<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, ShimFunction::from(self))
    }
}

impl<'db> ShimFunctionDiscriminator<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, ShimFunction::from(self))
    }
}

impl<'db> ShimFunctionPlainCoerce<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, ShimFunction::from(self))
    }
}

impl<'db> ShimFunctionDiscriminatedCoerce<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, ShimFunction::from(self))
    }
}

impl<'db> ShimParam<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiParam<'db> {
        FfiParam::new(db, self)
    }
}
