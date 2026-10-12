//! Intermediate representation of the ABI between the Rust runtime and
//! JavaScript.

pub mod ext;

use crate::compiler::{
    db::Db,
    ir::{
        interface::{InterClass, InterType, Interface},
        runtime::{RuntimeFunction, RuntimeParam},
    },
};

/// The ABI exports of a reachable interface API.
#[salsa::interned(debug)]
pub struct Abi<'db> {
    interface: Interface<'db>,
}

#[salsa::tracked]
impl<'db> Abi<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbols(self, db: &'db dyn Db) -> Vec<String> {
        let runtime = self.interface(db).runtime(db);

        let classes = runtime
            .classes(db)
            .iter()
            .map(|class| class.interface(db).abi(db).symbol(db));
        let functions = runtime
            .functions(db)
            .iter()
            .map(|function| function.abi(db).symbol(db));

        let symbols: std::collections::BTreeSet<_> = classes.chain(functions).collect();

        symbols.into_iter().map(str::to_owned).collect()
    }
}

#[salsa::interned(debug)]
pub struct AbiClass<'db> {
    interface: InterClass<'db>,
}

impl<'db> AbiClass<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.interface(db).runtime(db).symbol(db)
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
    pub fn params(self, db: &'db dyn Db) -> Vec<AbiParam<'db>> {
        self.runtime(db)
            .params(db)
            .iter()
            .map(|param| AbiParam::new(db, *param))
            .collect()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<AbiType<'db>> {
        self.runtime(db).ret_type(db).map(|type_| match type_ {
            InterType::f64 => AbiType::F64,
            InterType::String => AbiType::String,
            InterType::bool => AbiType::Bool,
            InterType::Class { is_borrowed, inner } => AbiType::Class {
                class: inner.abi(db),
                borrowed: is_borrowed,
            },
        })
    }
}

#[salsa::interned(debug)]
pub struct AbiParam<'db> {
    runtime: RuntimeParam<'db>,
}

impl<'db> AbiParam<'db> {
    pub fn type_(self, db: &'db dyn Db) -> AbiType<'db> {
        match self.runtime(db).type_ {
            InterType::f64 => AbiType::F64,
            InterType::String => AbiType::String,
            InterType::bool => AbiType::Bool,
            InterType::Class { is_borrowed, inner } => AbiType::Class {
                class: inner.abi(db),
                borrowed: is_borrowed,
            },
        }
    }
}

/// A logical type transferred across the ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum AbiType<'db> {
    F64,
    Bool,
    String,
    Class {
        class: AbiClass<'db>,
        borrowed: bool,
    },
}
