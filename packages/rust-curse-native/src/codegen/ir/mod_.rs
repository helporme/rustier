use crate::codegen::ir::item::{ItemKind, define_item_id};

define_item_id!(ModId, ItemKind::Mod, "Identifies a lowered Rust module.");

/// A module definition.
#[salsa::tracked(debug)]
pub struct Mod<'db> {
    pub id: ModId<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}
