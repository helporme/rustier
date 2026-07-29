use crate::codegen::{db::Db, ir::mod_::ModId, mangle::mangle_path};

/// Fully qualified item path.
#[salsa::interned(debug)]
pub struct ItemPath<'db> {
    #[returns(ref)]
    pub segments: Vec<String>,
}

/// Item kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum ItemKind {
    Struct,
    Enum,
    CLikeEnum,
    Mod,
    Fn,
}

/// Identifies a Rust item by its fully qualified path.
#[salsa::interned(debug)]
#[derive(Ord, PartialOrd)]
pub struct ItemId<'db> {
    /// Fully qualified item path including a crate name.
    #[returns(ref)]
    pub path: ItemPath<'db>,
    /// The kind of the item.
    #[returns(copy)]
    pub kind: ItemKind,
}

#[salsa::tracked]
impl<'db> ItemId<'db> {
    /// Returns the fully qualified item name, e.g. `geometry::Point`.
    #[salsa::tracked(returns(ref))]
    pub fn fqn(self, db: &'db dyn Db) -> String {
        self.path(db).segments(db).join("::")
    }

    /// Returns the unqualified item name, e.g. `Point`.
    #[salsa::tracked(returns(ref))]
    pub fn name(self, db: &'db dyn Db) -> String {
        self.path(db)
            .segments(db)
            .last()
            .expect("Item must have a name")
            .to_string()
    }

    /// Returns the mangled fully qualified item name, e.g. `geometry_Point`.
    #[salsa::tracked(returns(ref))]
    pub fn mangled_fqn(self, db: &'db dyn Db) -> String {
        mangle_path(self.path(db).segments(db))
    }

    /// Returns the item's exported JavaScript name.
    #[salsa::tracked(returns(ref))]
    pub fn extern_name(self, db: &'db dyn Db) -> String {
        format!("__ext_{}", self.mangled_fqn(db))
    }

    /// Returns the parent module ID, or `None` if this ID has no parent path
    /// segment.
    #[salsa::tracked]
    pub fn parent_mod_id(self, db: &'db dyn Db) -> Option<ModId<'db>> {
        let segments = &self.path(db).segments(db);

        if segments.len() <= 1 {
            return None;
        }

        // todo: result of this function invokation is always unwrapped since it's used
        // in a context where it's obvious that the function returns Some (like id of
        // fn, struct etc.). It might be worth considering adding a special ids like
        // FnId to avoid it.
        Some(
            ModId::new(
                db,
                ItemId::new(
                    db,
                    ItemPath::new(db, segments[..segments.len() - 1].to_vec()),
                    ItemKind::Mod,
                ),
            )
            .unwrap(),
        )
    }
}

/// A trait for converting an item into an [`ItemId`].
pub trait IntoItemId<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db>;
}

/// Provides common item ID operations over [`ItemId`].
pub trait ItemIdExt<'db> {
    /// Fully qualified item name, e.g. `kitties::FluffyCat`.
    fn fqn(self, db: &'db dyn Db) -> &'db String;

    /// Unqualified item name, e.g. `FluffyCat`.
    fn name(self, db: &'db dyn Db) -> &'db String;

    /// Mangled fully qualified item name, e.g. `kitties_FluffyCat`.
    fn mangled_fqn(self, db: &'db dyn Db) -> &'db String;

    /// Item's exported JavaScript name.
    fn extern_name(self, db: &'db dyn Db) -> &'db String;

    // todo: move to the ir/mod.rs
    /// Parent module ID, or `None` when the item has no parent.
    fn parent_mod_id(self, db: &'db dyn Db) -> Option<ModId<'db>>;
}

impl<'db, T: IntoItemId<'db>> ItemIdExt<'db> for T {
    fn fqn(self, db: &'db dyn Db) -> &'db String {
        self.into_item_id(db).fqn(db)
    }

    fn name(self, db: &'db dyn Db) -> &'db String {
        self.into_item_id(db).name(db)
    }

    fn mangled_fqn(self, db: &'db dyn Db) -> &'db String {
        self.into_item_id(db).mangled_fqn(db)
    }

    fn extern_name(self, db: &'db dyn Db) -> &'db String {
        self.into_item_id(db).extern_name(db)
    }

    fn parent_mod_id(self, db: &'db dyn Db) -> Option<ModId<'db>> {
        self.into_item_id(db).parent_mod_id(db)
    }
}

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

        impl<'db> $crate::codegen::ir::item::IntoItemId<'db> for $name<'db> {
            fn into_item_id(
                self,
                _db: &'db dyn $crate::codegen::db::Db,
            ) -> $crate::codegen::ir::item::ItemId<'db> {
                self.0
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
        }
    };
}

pub(crate) use define_item_id;
