use crate::codegen::{
    db::Db,
    item::{ItemId, ItemKind, ItemPath},
};

/// A module definition.
#[salsa::tracked(debug)]
pub struct Mod<'db> {
    pub id: ItemId<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

#[salsa::tracked]
impl<'db> ItemId<'db> {
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
