use crate::codegen::{db::Db, mangle::mangle_path};

macro_rules! define_item_id {
    ($name:ident, $kind:path, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, salsa::Update)]
        pub struct $name<'db>($crate::codegen::ir::item::ItemId<'db>);

        impl<'db> From<$name<'db>> for $crate::codegen::ir::item::ItemId<'db> {
            fn from(value: $name<'db>) -> Self {
                value.0
            }
        }

        impl<'db> $name<'db> {
            /// Creates an ID when the inner item has the expected kind.
            pub fn new(
                db: &'db dyn $crate::codegen::db::Db,
                inner: $crate::codegen::ir::item::ItemId<'db>,
            ) -> Option<Self> {
                if inner.kind(db) == $kind {
                    Some(Self(inner))
                } else {
                    None
                }
            }

            /// Returns the fully qualified item name.
            pub fn fqn(self, db: &'db dyn $crate::codegen::db::Db) -> &'db String {
                self.0.fqn(db)
            }

            /// Returns the unqualified item name.
            pub fn name(self, db: &'db dyn $crate::codegen::db::Db) -> &'db String {
                self.0.name(db)
            }

            /// Returns the mangled fully qualified item name.
            pub fn mangled_fqn(self, db: &'db dyn $crate::codegen::db::Db) -> &'db String {
                self.0.mangled_fqn(db)
            }

            /// Returns the rustdoc ID matching this item.
            pub fn rustdoc_id(
                self,
                db: &'db dyn $crate::codegen::db::Db,
            ) -> $crate::codegen::rustdoc::RustdocId<'db> {
                self.0.rustdoc_id(db)
            }

            /// Returns the parent module ID, if this item has a parent path segment.
            pub fn parent_mod_id(
                self,
                db: &'db dyn $crate::codegen::db::Db,
            ) -> Option<$crate::codegen::ir::item::ItemId<'db>> {
                self.0.parent_mod_id(db)
            }
        }
    };
}

pub(crate) use define_item_id;

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

    /// Returns the parent module ID, or `None` if this ID has no parent path
    /// segment.
    #[salsa::tracked]
    pub fn parent_mod_id(self, db: &'db dyn Db) -> Option<ItemId<'db>> {
        let segments = &self.path(db).segments;

        if segments.len() <= 1 {
            return None;
        }

        // todo: result of this function invokation is always unwrapped since it's used
        // in a context where it's obvious that the function returns Some (like id of
        // fn, struct etc.). It might be worth considering adding a special ids like
        // FnId to avoid it.
        Some(ItemId::new(
            db,
            ItemPath {
                segments: segments[..segments.len() - 1].to_vec(),
            },
            ItemKind::Mod,
        ))
    }
}

pub trait ItemIdExt<'db> {
    fn fqn(self, db: &'db dyn Db) -> &'db String;
    fn name(self, db: &'db dyn Db) -> &'db String;
    fn mangled_fqn(self, db: &'db dyn Db) -> &'db String;
    fn parent_mod_id(self, db: &'db dyn Db) -> Option<ItemId<'db>>;
}

impl<'db, T: Into<ItemId<'db>>> ItemIdExt<'db> for T {
    fn fqn(self, db: &'db dyn Db) -> &'db String {
        self.into().fqn(db)
    }

    fn name(self, db: &'db dyn Db) -> &'db String {
        self.into().name(db)
    }

    fn mangled_fqn(self, db: &'db dyn Db) -> &'db String {
        self.into().mangled_fqn(db)
    }

    fn parent_mod_id(self, db: &'db dyn Db) -> Option<ItemId<'db>> {
        self.into().parent_mod_id(db)
    }
}
