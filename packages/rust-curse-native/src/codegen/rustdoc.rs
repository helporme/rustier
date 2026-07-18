use std::{collections::BTreeMap, path::PathBuf};

use crate::codegen::db::{Db, DbImpl};
use rustdoc_types::{Crate as RCrate, Id as RId, Item as RItem, Type};
use salsa::Setter;

pub mod enum_;
pub mod fn_;
pub mod mod_;
pub mod shape;
pub mod struct_;
pub mod type_;

#[salsa::input(singleton)]
pub struct RustdocCrates {
    #[returns(ref)]
    pub crates: BTreeMap<String, Crate>,
}

impl RustdocCrates {
    /// Sets the active rustdoc crates in the database.
    ///
    /// # Example
    ///
    /// ```
    /// use std::collections::BTreeMap;
    ///
    /// use rust_curse::codegen::{db::DbImpl, rustdoc::RustdocCrates};
    ///
    /// let mut db = DbImpl::default();
    /// RustdocCrates::set(&mut db, BTreeMap::new());
    ///
    /// assert!(RustdocCrates::get(&db).crates(&db).is_empty());
    /// ```
    pub fn set(db: &mut DbImpl, crates: BTreeMap<String, Crate>) -> Self {
        if let Some(rustdoc) = Self::try_get(db) {
            rustdoc.set_crates(db).to(crates);
            rustdoc
        } else {
            Self::new(db, crates)
        }
    }

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
        let rustdoc = RustdocCrates::get(db);
        rustdoc.crate_data(db, self.crate_name(db)).unwrap()
    }

    /// Find the crate where this type is defined.
    pub fn crate_(&self, db: &'db dyn Db) -> &'db RCrate {
        &self.crate_data(db).body
    }
}

/// An interned ID to a specific rustdoc item in a specific crate.
#[salsa::interned(debug)]
pub struct RustdocId<'db> {
    #[returns(copy)]
    pub idx: RId,
    #[returns(ref)]
    pub crate_name: String,
}

impl<'db> RustdocId<'db> {
    /// Find the rustdoc item corresponding to this ID, from the crate stored in
    /// database.
    pub fn item(&self, db: &'db dyn Db) -> &RItem {
        let crate_data = self.crate_(db);
        let idx = self.idx(db);

        crate_data.body.index.get(&idx).expect("item not found")
    }

    /// Find the crate data where this item with this id is defined.
    pub fn crate_(&self, db: &'db dyn Db) -> &'db Crate {
        let rustdoc = RustdocCrates::get(db);
        rustdoc
            .crate_data(db, self.crate_name(db))
            .expect("crate not found")
    }

    /// Find the crate where this item with this id is defined.
    pub fn crate_body(&self, db: &'db dyn Db) -> &'db RCrate {
        &self.crate_(db).body
    }
}

/// Extension trait for [`RCrate`] that provides convenience methods for code
/// generation.
pub trait CrateExt {
    /// Returns the [`RustdocId`] for the given `id` in this crate.
    fn id<'db>(&self, db: &'db dyn Db, id: RId) -> RustdocId<'db>;

    /// Returns the [`RustdocType`] for the given `ty` in this crate.
    fn ty<'db>(&self, db: &'db dyn Db, ty: &Type) -> RustdocType<'db>;
}

impl CrateExt for RCrate {
    fn id<'db>(&self, db: &'db dyn Db, id: RId) -> RustdocId<'db> {
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

#[derive(Clone, Debug, PartialEq)]
pub struct Crate {
    /// The parsed rustdoc crate body.
    pub body: RCrate,
    /// Whether the crate is external (i.e. not part of the workspace).
    pub is_external: bool,
    /// The crate directory absolute path.
    pub dir: PathBuf,
}
