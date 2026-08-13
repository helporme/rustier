use crate::compiler::{
    db::Db,
    ir::shim::{ShimClass, ShimFunction, ShimParam},
};

#[salsa::interned(debug)]
pub struct FfiClass<'db> {
    shim: ShimClass<'db>,
}

impl<'db> FfiClass<'db> {
    pub fn as_type(self) -> FfiType<'db> {
        FfiType::Class(self)
    }

    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.shim(db).symbol(db)
    }
}

#[salsa::interned(debug)]
pub struct FfiFunction<'db> {
    shim: ShimFunction<'db>,
}

#[salsa::tracked]
impl<'db> FfiFunction<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.shim(db).symbol(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<FfiParam<'db>> {
        self.shim(db)
            .params(db)
            .iter()
            .copied()
            .map(|param| param.ffi(db))
            .collect()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<FfiType<'db>> {
        self.shim(db).ret_type(db).map(|type_| type_.ffi(db))
    }
}

#[salsa::interned(debug)]
pub struct FfiParam<'db> {
    shim: ShimParam<'db>,
}

impl<'db> FfiParam<'db> {
    pub fn type_(self, db: &'db dyn Db) -> FfiType<'db> {
        self.shim(db).type_(db).ffi(db)
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
