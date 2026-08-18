use crate::compiler::{
    db::Db,
    ir::{
        ffi::{FfiClass, FfiFunction, FfiParam, FfiType},
        shim::{
            ShimClass, ShimFunction, ShimFunctionEnumDiscriminator, ShimFunctionFree,
            ShimFunctionGetter, ShimFunctionSetter, ShimParam, ShimType,
        },
    },
};

impl<'db> ShimClass<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        FfiClass::new(db, self)
    }
}

impl<'db> ShimFunction<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, self)
    }
}

impl<'db> ShimFunctionFree<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.as_fn().ffi(db)
    }
}

impl<'db> ShimFunctionGetter<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.as_fn().ffi(db)
    }
}

impl<'db> ShimFunctionSetter<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.as_fn().ffi(db)
    }
}

impl<'db> ShimFunctionEnumDiscriminator<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.as_fn().ffi(db)
    }
}

impl<'db> ShimParam<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiParam<'db> {
        FfiParam::new(db, self)
    }
}

impl<'db> ShimType<'db> {
    pub(super) fn lower_ffi(self, db: &'db dyn Db) -> FfiType<'db> {
        match self {
            ShimType::f64 => FfiType::number,
            ShimType::String => FfiType::string,
            ShimType::bool => FfiType::boolean,
            ShimType::Class { inner, .. } => FfiType::Class(inner.ffi(db)),
        }
    }
}
