//! Intermediate representation of the ABI between the Rust runtime and
//! JavaScript.

pub mod backend;
pub mod ext;

use crate::compiler::{
    db::Db,
    ir::{facade::Facade, runtime::RuntimeFunction},
};

/// The ABI exports of a reachable interface API.
#[salsa::interned(debug)]
pub struct Transport<'db> {
    pub facade: Facade<'db>,
}

#[salsa::tracked]
impl<'db> Transport<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbols(self, db: &'db dyn Db) -> Vec<String> {
        let runtime = self.facade(db).runtime(db);

        let functions = runtime
            .functions(db)
            .iter()
            .map(|function| function.transport(db).symbol(db));

        let symbols: std::collections::BTreeSet<_> = functions.collect();

        symbols.into_iter().map(str::to_owned).collect()
    }
}

#[salsa::interned(debug)]
pub struct TransportFunction<'db> {
    runtime: RuntimeFunction<'db>,
}

#[salsa::tracked]
impl<'db> TransportFunction<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.runtime(db).symbol(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<TransportType> {
        self.runtime(db)
            .params(db)
            .iter()
            .map(|type_| type_.transport(db))
            .collect()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> TransportReturn<'db> {
        self.runtime(db).transport_ret(db)
    }

    pub fn uses_js_values(self, db: &'db dyn Db) -> bool {
        if self.params(db).contains(&TransportType::JsValueHandle) {
            return true;
        }

        match self.ret_type(db) {
            TransportReturn::Value(type_) => type_ == TransportType::JsValueHandle,
            TransportReturn::Union(union) => {
                union.variants(db).contains(&TransportType::JsValueHandle)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TransportReturn<'db> {
    Value(TransportType),
    // There is currently no need for transport to support union as input type (e.g as argument),
    // however once we will need it, the TransportReturn should be removed and Union should be
    // placed as TransportType variant.
    Union(TransportUnion<'db>),
}

#[salsa::interned(debug)]
pub struct TransportUnion<'db> {
    #[returns(ref)]
    pub variants: Vec<TransportType>,
}

/// A logical type transferred across the ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum TransportType {
    Unit,
    f64,
    bool,
    String,
    usize,
    JsValueHandle,
}
