use std::collections::BTreeMap;

use crate::codegen::{db::Db, ir};
use rustdoc_types::{Crate as RCrate, Id, Item as RItem, Span, Type};

mod fn_;
mod type_;

pub use fn_::*;
pub use type_::*;

#[salsa::input(singleton)]
pub struct Rustdoc {
    #[returns(ref)]
    pub crates: BTreeMap<String, RCrate>,
}

impl Rustdoc {
    pub fn crate_<'db>(&self, db: &'db dyn Db, name: &str) -> Option<&'db RCrate> {
        self.crates(db).get(name)
    }
}

/// An interned type to a specific rustdoc item in a specific crate.
#[salsa::interned]
pub struct RustdocType<'db> {
    #[returns(ref)]
    pub type_: Type,
    #[returns(ref)]
    pub crate_name: String,
}

impl<'db> RustdocType<'db> {
    /// Find the crate where this item with this id is defined.
    pub fn crate_(&self, db: &'db dyn Db) -> &'db RCrate {
        let rustdoc = Rustdoc::get(db);
        rustdoc.crate_(db, self.crate_name(db)).unwrap()
    }
}

/// An interned ID to a specific rustdoc item in a specific crate.
#[salsa::interned]
pub struct RustdocId<'db> {
    #[returns(copy)]
    pub idx: Id,
    #[returns(ref)]
    pub crate_name: String,
}

impl<'db> RustdocId<'db> {
    /// Find the rustdoc item corresponding to this ID, from the crate stored in
    /// database.
    pub fn item(&self, db: &'db dyn Db) -> &RItem {
        let rustdoc = Rustdoc::get(db);
        let crate_name = self.crate_name(db);
        let idx = self.idx(db);

        let crate_ = rustdoc.crate_(db, crate_name).expect("crate not found");
        let item = crate_.index.get(&idx).expect("item not found");

        item
    }

    /// Find the crate where this item with this id is defined.
    pub fn crate_(&self, db: &'db dyn Db) -> &'db RCrate {
        let rustdoc = Rustdoc::get(db);
        rustdoc.crate_(db, self.crate_name(db)).unwrap()
    }
}

pub trait CrateExt {
    /// Returns the [`RustdocId`] for the given `id` in this crate.
    fn id<'db>(&self, db: &'db dyn Db, id: Id) -> RustdocId<'db>;

    /// Returns the [`RustdocType`] for the given `ty` in this crate.
    fn ty<'db>(&self, db: &'db dyn Db, ty: &Type) -> RustdocType<'db>;
}

impl CrateExt for RCrate {
    fn id<'db>(&self, db: &'db dyn Db, id: Id) -> RustdocId<'db> {
        let crate_name = self
            .index
            .get(&self.root)
            .and_then(|item| item.name.as_deref())
            .expect("root module has no name");

        RustdocId::new(db, id, crate_name)
    }

    fn ty<'db>(&self, db: &'db dyn Db, ty: &Type) -> RustdocType<'db> {
        let crate_name = self
            .index
            .get(&self.root)
            .and_then(|item| item.name.as_deref())
            .expect("root module has no name");

        RustdocType::new(db, ty, crate_name)
    }
}

impl From<Span> for ir::Span {
    fn from(span: Span) -> Self {
        ir::Span {
            filename: span.filename,
            begin: span.begin,
            end: span.end,
        }
    }
}
