//! Intermediate representation of the API actually exposed to JavaScript.
//!
//! It represents an actual and complete API that JavaScript receives from the
//! rust code. All other generated JavaScript code is a "facade" on top of it.

pub mod ext;
mod lowering;

use crate::compiler::{
    db::Db,
    ir::{
        interface::{InterClass, Interface},
        runtime::{RuntimeFunction, RuntimeParam},
    },
};

/// The actual exports of a reachable interface API.
#[salsa::interned(debug)]
pub struct Ffi<'db> {
    interface: Interface<'db>,
}

#[salsa::tracked]
impl<'db> Ffi<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbols(self, db: &'db dyn Db) -> Vec<String> {
        let runtime = self.interface(db).runtime(db);

        let classes = runtime
            .classes(db)
            .iter()
            .map(|class| class.interface(db).ffi(db).symbol(db));
        let functions = runtime
            .functions(db)
            .iter()
            .map(|function| function.ffi(db).symbol(db));

        let symbols: std::collections::BTreeSet<_> = classes.chain(functions).collect();

        symbols.into_iter().map(str::to_owned).collect()
    }
}

#[salsa::interned(debug)]
pub struct FfiClass<'db> {
    interface: InterClass<'db>,
}

impl<'db> FfiClass<'db> {
    pub fn as_type(self) -> FfiType<'db> {
        FfiType::Class(self)
    }

    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.interface(db).runtime(db).symbol(db)
    }
}

#[salsa::interned(debug)]
pub struct FfiFunction<'db> {
    runtime: RuntimeFunction<'db>,
}

impl<'db> FfiFunction<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.runtime(db).symbol(db)
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
    runtime: RuntimeParam<'db>,
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
