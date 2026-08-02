use crate::codegen::{
    db::Db,
    ir::{
        enum_::Enum,
        fn_::Fn,
        item::{IntoItemId, Item, ItemId, ItemKind, define_item_id},
        span::Span,
        struct_::Struct,
    },
};

define_item_id!(ModId, ItemKind::Mod, "Identifies a lowered Rust module.");

/// A lowered Rust module.
#[salsa::tracked(debug)]
pub struct Mod<'db> {
    pub id: ModId<'db>,
    /// Items defined directly inside this module.
    #[tracked]
    #[returns(ref)]
    pub items: Vec<Item<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

impl<'db> Mod<'db> {
    pub fn fns(self, db: &'db dyn Db) -> impl Iterator<Item = Fn<'db>> + Clone + 'db {
        self.items(db).iter().filter_map(|item| match item {
            Item::Fn(fn_) => Some(*fn_),
            _ => None,
        })
    }

    pub fn structs(self, db: &'db dyn Db) -> impl Iterator<Item = Struct<'db>> + Clone + 'db {
        self.items(db).iter().filter_map(|item| match item {
            Item::Struct(struct_) => Some(*struct_),
            _ => None,
        })
    }

    pub fn enums(self, db: &'db dyn Db) -> impl Iterator<Item = Enum<'db>> + Clone + 'db {
        self.items(db).iter().filter_map(|item| match item {
            Item::Enum(enum_) => Some(*enum_),
            _ => None,
        })
    }
}

impl<'db> IntoItemId<'db> for Mod<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        self.id(db).into()
    }
}
