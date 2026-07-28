use crate::codegen::ir::item::ItemId;

/// A module definition.
#[salsa::tracked(debug)]
pub struct Mod<'db> {
    pub id: ItemId<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}
