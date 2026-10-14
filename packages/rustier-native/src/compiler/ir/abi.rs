//! Intermediate representation of the ABI between the Rust runtime and
//! JavaScript.

pub mod ext;
pub mod transport;

use crate::compiler::{
    db::Db,
    ir::{
        facade::Facade,
        runtime::{RuntimeFunction, RuntimeType},
        rust::{Type, TypeKind},
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
            .map(|type_| AbiType::new(db, *type_))
            .collect()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<AbiType> {
        self.runtime(db).ret_type(db).map(|type_| match type_ {
            RuntimeType::Source(type_) => AbiType::new(db, type_),
            RuntimeType::u32 => AbiType::f64,
            RuntimeType::Tethered(_) => AbiType::usize,
        })
    }
}

/// A logical type transferred across the ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum AbiType {
    f64,
    bool,
    String,
    usize,
}

impl AbiType {
    fn new<'db>(db: &'db dyn Db, type_: Type<'db>) -> Self {
        match type_.kind(db) {
            TypeKind::bool => Self::bool,
            TypeKind::char
            | TypeKind::i8
            | TypeKind::u8
            | TypeKind::i16
            | TypeKind::u16
            | TypeKind::i32
            | TypeKind::u32
            | TypeKind::f32
            | TypeKind::f64 => Self::f64,
            TypeKind::Str | TypeKind::String => Self::String,
            TypeKind::Borrowed(borrowed_type) => Self::new(db, borrowed_type.inner(db)),
            TypeKind::Struct(_) | TypeKind::Enum(_) => Self::usize,
            TypeKind::Generic(_) => {
                unreachable!("unsupported Rust types do not cross the ABI")
            }
        }
    }
}
