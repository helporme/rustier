use crate::codegen::{db::Db, mangle::mangle_path};

/// Fully qualified item path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ItemPath {
    pub segments: Vec<String>,
}

/// Item kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum ItemKind {
    Struct,
    Enum,
    Mod,
    Fn,
}

/// Identifies a Rust item by its fully qualified path.
#[salsa::interned(debug)]
#[derive(Ord, PartialOrd)]
pub struct ItemId<'db> {
    /// Fully qualified item path.
    #[returns(ref)]
    pub path: ItemPath,
    /// The kind of the item.
    #[returns(copy)]
    pub kind: ItemKind,
}

#[salsa::tracked]
impl<'db> ItemId<'db> {
    /// Returns the fully qualified item name, e.g. `geometry::Point`.
    #[salsa::tracked(returns(ref))]
    pub fn fqn(self, db: &'db dyn Db) -> String {
        self.path(db).segments.join("::")
    }

    /// Returns the unqualified item name, e.g. `Point`.
    #[salsa::tracked(returns(ref))]
    pub fn name(self, db: &'db dyn Db) -> String {
        self.path(db)
            .segments
            .last()
            .expect("Item must have a name")
            .to_string()
    }

    /// Returns the mangled fully qualified item name, e.g. `geometry_Point`.
    #[salsa::tracked(returns(ref))]
    pub fn mangled_fqn(self, db: &'db dyn Db) -> String {
        mangle_path(&self.path(db).segments)
    }
}
