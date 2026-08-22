//! Intermediate representation of the API actually exposed to JavaScript.
//!
//! It represents an actual and complete API that JavaScript receives from the
//! rust code. All other generated JavaScript code is a "facade" on top of it.

pub mod ext;
mod lowering;

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

impl<'db> FfiFunction<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.shim(db).symbol(db)
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<FfiParam<'db>> {
        self.lower_params(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<FfiType<'db>> {
        self.lower_ret_type(db)
    }
}

#[salsa::interned(debug)]
pub struct FfiParam<'db> {
    shim: ShimParam<'db>,
}

impl<'db> FfiParam<'db> {
    pub fn type_(self, db: &'db dyn Db) -> FfiType<'db> {
        self.lower_type(db)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum FfiType<'db> {
    boolean,
    string,
    number,
    Class(FfiClass<'db>),
}
