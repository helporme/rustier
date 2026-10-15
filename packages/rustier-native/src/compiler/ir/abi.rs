//! Intermediate representation of the ABI between the Rust runtime and
//! JavaScript.

pub mod ext;
pub mod transport;

use crate::compiler::{
    db::Db,
    ir::{
        facade::Facade,
        runtime::{RuntimeFunction, RuntimeType},
    },
};

/// The ABI exports of a reachable interface API.
#[salsa::interned(debug)]
pub struct Abi<'db> {
    facade: Facade<'db>,
}

#[salsa::tracked]
impl<'db> Abi<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbols(self, db: &'db dyn Db) -> Vec<String> {
        let runtime = self.facade(db).runtime(db);

        let functions = runtime
            .functions(db)
            .iter()
            .map(|function| function.abi(db).symbol(db));

        let symbols: std::collections::BTreeSet<_> = functions.collect();

        symbols.into_iter().map(str::to_owned).collect()
    }
}

#[salsa::interned(debug)]
pub struct AbiFunction<'db> {
    runtime: RuntimeFunction<'db>,
}

#[salsa::tracked]
impl<'db> AbiFunction<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.runtime(db).symbol(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<AbiType> {
        self.runtime(db)
            .params(db)
            .iter()
            .map(|type_| type_.abi(db))
            .collect()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> AbiReturn<'db> {
        let type_ = match self.runtime(db).ret_type(db) {
            Some(RuntimeType::Source(type_)) => type_.abi(db),
            Some(RuntimeType::u32) => AbiType::f64,
            Some(RuntimeType::Tethered(_)) => AbiType::usize,
            None => AbiType::Unit,
        };

        AbiReturn::Value(type_)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum AbiReturn<'db> {
    Value(AbiType),
    Union(AbiUnion<'db>),
}

#[salsa::interned(debug)]
pub struct AbiUnion<'db> {
    #[returns(ref)]
    pub variants: Vec<AbiType>,
}

/// A logical type transferred across the ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum AbiType {
    Unit,
    f64,
    bool,
    String,
    usize,
}
