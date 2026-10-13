//! Intermediate representation of the ABI between the Rust runtime and
//! JavaScript.

pub mod ext;

use crate::compiler::{
    db::Db,
    ir::{
        interface::{InterClass, Interface},
        runtime::{RuntimeFunction, RuntimeType},
        rust::{Type, TypeKind},
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
    pub interface: InterClass<'db>,
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
    pub fn params(self, db: &'db dyn Db) -> Vec<AbiType<'db>> {
        self.runtime(db)
            .params(db)
            .iter()
            .map(|type_| AbiType::new(db, *type_, true))
            .collect()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<AbiType<'db>> {
        self.runtime(db).ret_type(db).map(|type_| match type_ {
            RuntimeType::Source(type_) => AbiType::new(db, type_, false),
            RuntimeType::u32 => AbiType::F64,
        })
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

impl<'db> AbiType<'db> {
    fn new(db: &'db dyn Db, type_: Type<'db>, borrowed: bool) -> Self {
        if let Some(class) = type_.interface_class(db) {
            return Self::Class {
                class: class.abi(db),
                borrowed,
            };
        }

        match type_.kind(db) {
            TypeKind::bool => Self::Bool,
            TypeKind::char
            | TypeKind::i8
            | TypeKind::u8
            | TypeKind::i16
            | TypeKind::u16
            | TypeKind::i32
            | TypeKind::u32
            | TypeKind::f32
            | TypeKind::f64 => Self::F64,
            TypeKind::Str | TypeKind::String => Self::String,
            TypeKind::Borrowed(borrowed_type) => Self::new(db, borrowed_type.inner(db), borrowed),
            TypeKind::Struct(_) | TypeKind::Enum(_) | TypeKind::Generic(_) => {
                unreachable!("unsupported Rust types do not cross the ABI")
            }
        }
    }
}
