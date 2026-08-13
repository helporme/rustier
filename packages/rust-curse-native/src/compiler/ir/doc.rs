use std::{collections::BTreeMap, path::PathBuf};

use crate::compiler::db::{Db, DbImpl};
use rustdoc_types as doc;
use salsa::Setter as _;

/// Parsed rustdoc crates used as input for code generation.
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
    /// use rust_curse::compiler::{db::DbImpl, ir::doc::RustdocCrates};
    ///
    /// let mut db = DbImpl::default();
    /// RustdocCrates::set(&mut db, BTreeMap::new());
    ///
    /// assert!(RustdocCrates::get(&db).crates(&db).is_empty());
    /// ```
    #[allow(dead_code)]
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

    pub fn crate_<'db>(&self, db: &'db dyn Db, name: &str) -> Option<&'db doc::Crate> {
        self.crate_data(db, name).map(|c| &c.body)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Crate {
    /// The parsed rustdoc crate body.
    pub body: doc::Crate,
    /// Whether the crate is external (i.e. not part of the workspace).
    pub is_external: bool,
    /// The crate directory absolute path.
    pub dir: PathBuf,
}
