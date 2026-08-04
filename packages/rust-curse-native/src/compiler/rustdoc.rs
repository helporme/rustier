use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

use crate::compiler::{
    db::{Db, DbImpl},
    ir::{IntoItemId, ItemId, ItemKind, ItemPath},
};
use rustdoc_types::{Crate as RCrate, Id as RId, Item as RItem, ItemKind as RItemKind, Type};
use salsa::Setter;

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
    /// use rust_curse::compiler::{db::DbImpl, rustdoc::RustdocCrates};
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

#[salsa::tracked]
impl<'db> RustdocId<'db> {
    /// Find the rustdoc item corresponding to the given path, if one exists.
    pub fn find_by_path(db: &'db dyn Db, path: &ItemPath) -> Option<Self> {
        let backmap = rustdoc_backmap(db);
        backmap.get(path).copied()
    }

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

    #[salsa::tracked]
    pub fn item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        let crate_ = self.crate_body(db);
        let ritem = crate_.paths.get(&self.idx(db)).unwrap();

        ItemId::new(
            db,
            ItemPath::new(db, &ritem.path),
            ItemKind::from(ritem.kind),
        )
    }
}

#[salsa::tracked]
impl<'db> ItemId<'db> {
    /// Returns the rustdoc ID matching an IR item, or panics if no match is
    /// found.
    #[salsa::tracked]
    pub(crate) fn rustdoc_id(self, db: &'db dyn Db) -> RustdocId<'db> {
        let backmap = rustdoc_backmap(db);

        // todo: remove panic
        *backmap
            .get(self.path(db))
            .unwrap_or_else(|| panic!("no rustdoc item for `{}`", self.fqn(db)))
    }
}

/// Provides common item ID operations over [`ItemId`].
pub trait ItemIdExt<'db> {
    fn rustdoc_id(self, db: &'db dyn Db) -> RustdocId<'db>;
}

impl<'db, T: IntoItemId<'db>> ItemIdExt<'db> for T {
    fn rustdoc_id(self, db: &'db dyn Db) -> RustdocId<'db> {
        self.into_item_id(db).rustdoc_id(db)
    }
}

#[salsa::tracked(returns(ref))]
fn rustdoc_backmap<'db>(db: &'db dyn Db) -> HashMap<ItemPath<'db>, RustdocId<'db>> {
    let rustdoc = RustdocCrates::get(db);

    let mut backmap = HashMap::new();

    for (crate_name, crate_) in rustdoc.crates(db) {
        for (idx, item_summary) in &crate_.body.paths {
            if !crate_.body.index.contains_key(idx) {
                continue;
            }

            backmap.insert(
                ItemPath::new(db, &item_summary.path),
                RustdocId::new(db, *idx, crate_name),
            );
        }
    }

    backmap
}

impl From<RItemKind> for ItemKind {
    fn from(value: RItemKind) -> Self {
        match value {
            RItemKind::Struct => ItemKind::Struct,
            RItemKind::Enum => ItemKind::Enum,
            RItemKind::Module => ItemKind::Mod,
            RItemKind::Function => ItemKind::Fn,
            _ => unreachable!(),
        }
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
