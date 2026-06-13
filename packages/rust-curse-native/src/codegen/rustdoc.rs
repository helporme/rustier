use std::collections::BTreeMap;

use crate::{
    codegen::{db::Db, ir},
    tools::rustdoc::Crate,
};
use rustdoc_types::{Crate as RCrate, Id as RId, Item as RItem, Span, Type};

mod fn_;
mod mod_;
mod type_;

pub use fn_::*;
pub use mod_::*;
pub use type_::*;

#[salsa::input(singleton)]
pub struct Input {
    #[returns(ref)]
    pub crates: BTreeMap<String, Crate>,
}

impl Input {
    pub fn crate_data<'db>(&self, db: &'db dyn Db, name: &str) -> Option<&'db Crate> {
        self.crates(db).get(name)
    }

    pub fn crate_<'db>(&self, db: &'db dyn Db, name: &str) -> Option<&'db RCrate> {
        self.crate_data(db, name).map(|c| &c.body)
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
    /// Find the crate data where this type is defined.
    pub fn crate_data(&self, db: &'db dyn Db) -> &'db Crate {
        let rustdoc = Input::get(db);
        rustdoc.crate_data(db, self.crate_name(db)).unwrap()
    }

    /// Find the crate where this type is defined.
    pub fn crate_(&self, db: &'db dyn Db) -> &'db RCrate {
        &self.crate_data(db).body
    }
}

/// An interned ID to a specific rustdoc item in a specific crate.
#[salsa::interned]
pub struct Id<'db> {
    #[returns(copy)]
    pub idx: RId,
    #[returns(ref)]
    pub crate_name: String,
}

impl<'db> Id<'db> {
    /// Find the rustdoc item corresponding to this ID, from the crate stored in
    /// database.
    pub fn item(&self, db: &'db dyn Db) -> &RItem {
        let crate_data = self.crate_(db);
        let idx = self.idx(db);

        crate_data.body.index.get(&idx).expect("item not found")
    }

    /// Find the crate data where this item with this id is defined.
    pub fn crate_(&self, db: &'db dyn Db) -> &'db Crate {
        let rustdoc = Input::get(db);
        rustdoc
            .crate_data(db, self.crate_name(db))
            .expect("crate not found")
    }

    /// Find the crate where this item with this id is defined.
    pub fn crate_body(&self, db: &'db dyn Db) -> &'db RCrate {
        &self.crate_(db).body
    }

    pub fn span(&self, db: &'db dyn Db, span: Span) -> ir::Span {
        self.crate_(db).conv_span(span)
    }
}

/// Extension trait for [`RCrate`] that provides convenience methods for code
/// generation.
pub trait CrateExt {
    /// Returns the [`RustdocId`] for the given `id` in this crate.
    fn id<'db>(&self, db: &'db dyn Db, id: RId) -> Id<'db>;

    /// Returns the [`RustdocType`] for the given `ty` in this crate.
    fn ty<'db>(&self, db: &'db dyn Db, ty: &Type) -> RustdocType<'db>;
}

impl CrateExt for RCrate {
    fn id<'db>(&self, db: &'db dyn Db, id: RId) -> Id<'db> {
        let crate_name = self
            .index
            .get(&self.root)
            .and_then(|item| item.name.as_deref())
            .expect("root module has no name");

        Id::new(db, id, crate_name)
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
