//! Intermediate representation of the API actually exposed to JavaScript.
//!
//! It represents an actual and complete API that JavaScript receives from the
//! rust code. All other generated JavaScript code is a "facade" on top of it.

pub mod ext;
mod lowering;

use itertools::Either;

use crate::compiler::{
    db::Db,
    ir::interface::{InterClass, InterFunction, InterMember, InterParam, Interface},
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
        let mut symbols = std::collections::BTreeSet::new();

        for member in self
            .interface(db)
            .crates(db)
            .flat_map(|crate_| crate_.all_modules(db))
            .flat_map(|module| module.members(db))
        {
            let classes = match member {
                InterMember::Class(class) => Either::Left(std::iter::once(class)),
                InterMember::Template(template) => Either::Right(template.classes(db)),
                InterMember::Function(function) => {
                    symbols.insert(function.ffi(db).symbol(db));

                    continue;
                }
                InterMember::Module(_) => continue,
            };

            for class in classes {
                symbols.insert(class.ffi(db).symbol(db));
                symbols.extend(class.all_functions(db).iter().map(|f| f.ffi(db).symbol(db)));
            }
        }

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
        self.interface(db).symbol(db)
    }
}

#[salsa::interned(debug)]
pub struct FfiFunction<'db> {
    interface: InterFunction<'db>,
}

impl<'db> FfiFunction<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.interface(db).symbol(db)
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
    interface: InterParam<'db>,
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
