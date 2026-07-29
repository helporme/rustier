use crate::codegen::{
    db::Db,
    ir::item::{IntoItemId, ItemId, ItemKind, define_item_id},
};

define_item_id!(
    CLikeEnumId,
    ItemKind::CLikeEnum,
    "Identifies a lowered Rust c-like enum."
);

/// A lowered Rust enum that contains only unit variants.
#[salsa::tracked(debug)]
pub struct CLikeEnum<'db> {
    pub id: CLikeEnumId<'db>,
    #[tracked]
    #[returns(ref)]
    pub variants: Vec<CLikeEnumVariant<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

impl<'db> IntoItemId<'db> for CLikeEnum<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        self.id(db).into()
    }
}

/// A lowered fieldless enum variant and its optional explicit discriminant.
#[salsa::tracked(debug)]
pub struct CLikeEnumVariant<'db> {
    #[returns(ref)]
    pub name: String,
    /// The discriminant value of this variant as a number between [`i128::MIN`]
    /// and [`u128::MAX`], if any.
    #[tracked]
    #[returns(ref)]
    pub discriminant: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}
