use crate::compiler::{
    db::Db,
    ir::{
        facade::FacadeType,
        shim::{ShimClass, ShimCrate, ShimFunction, ShimParam},
    },
};

#[salsa::interned(debug)]
pub struct FfiCrate<'db> {
    shim: ShimCrate<'db>,
}

impl<'db> FfiCrate<'db> {
    pub fn fns(self, db: &'db dyn Db) -> &Vec<FfiFunction<'db>> {
        todo!()
    }
}

#[salsa::interned(debug)]
pub struct FfiClass<'db> {
    shim: ShimClass<'db>,
}

impl<'db> FfiClass<'db> {
    pub fn as_type(self) -> FfiType<'db> {
        FfiType::Class(self)
    }

    pub fn symbol(self, db: &'db dyn Db) -> &str {
        todo!()
    }
}

#[salsa::interned(debug)]
pub struct FfiFunction<'db> {
    shim: ShimFunction<'db>,
}

impl<'db> FfiFunction<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &str {
        todo!()
    }

    pub fn params(self, db: &'db dyn Db) -> &Vec<FfiParam<'db>> {
        todo!()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> FfiType<'db> {
        todo!()
    }
}

#[salsa::interned(debug)]
pub struct FfiParam<'db> {
    shim: ShimParam<'db>,
}

impl<'db> FfiParam<'db> {
    pub fn type_(self, db: &'db dyn Db) -> FfiType<'db> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum FfiType<'db> {
    bool,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    f64,
    String,
    Class(FfiClass<'db>),
}
