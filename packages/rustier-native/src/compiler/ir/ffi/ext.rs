use crate::compiler::{
    db::Db,
    ir::{
        ffi::{FfiClass, FfiFunction, FfiParam},
        interface::{
            InterClass, InterClassDiscriminated, InterClassPlain, InterFunction,
            InterFunctionAssociated, InterFunctionDiscriminatedCoerce, InterFunctionDiscriminator,
            InterFunctionFree, InterFunctionGetter, InterFunctionPlainCoerce, InterFunctionSetter,
            InterParam,
        },
    },
};

impl<'db> InterClass<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        FfiClass::new(db, self)
    }
}

impl<'db> InterClassPlain<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        FfiClass::new(db, InterClass::Plain(self))
    }
}

impl<'db> InterClassDiscriminated<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        FfiClass::new(db, InterClass::Discriminated(self))
    }
}

impl<'db> InterFunction<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, self)
    }
}

impl<'db> InterFunctionFree<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, InterFunction::from(self))
    }
}

impl<'db> InterFunctionAssociated<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, InterFunction::from(self))
    }
}

impl<'db> InterFunctionGetter<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, InterFunction::from(self))
    }
}

impl<'db> InterFunctionSetter<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, InterFunction::from(self))
    }
}

impl<'db> InterFunctionDiscriminator<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, InterFunction::from(self))
    }
}

impl<'db> InterFunctionPlainCoerce<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, InterFunction::from(self))
    }
}

impl<'db> InterFunctionDiscriminatedCoerce<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, InterFunction::from(self))
    }
}

impl<'db> InterParam<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiParam<'db> {
        FfiParam::new(db, self)
    }
}
