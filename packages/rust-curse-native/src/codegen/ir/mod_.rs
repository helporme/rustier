use crate::codegen::{
    db::Db,
    ir::item::{IntoItemId, ItemId, ItemKind, define_item_id},
};

define_item_id!(ModId, ItemKind::Mod, "Identifies a lowered Rust module.");

/// A module definition.
#[salsa::tracked(debug)]
pub struct Mod<'db> {
    pub id: ModId<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

impl<'db> IntoItemId<'db> for Mod<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        self.id(db).into()
    }
}
