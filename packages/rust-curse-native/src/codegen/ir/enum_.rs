use crate::codegen::{
    db::Db,
    ir::{
        item::{IntoItemId, ItemId, ItemKind, define_item_id},
        shape::{Shape, ShapeField},
    },
};

define_item_id!(EnumId, ItemKind::Enum, "Identifies a lowered Rust enum.");

/// A lowered Rust enum and its variants.
#[salsa::tracked(debug)]
pub struct Enum<'db> {
    pub id: EnumId<'db>,
    #[tracked]
    #[returns(ref)]
    pub variants: Vec<EnumVariant<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    pub is_non_exhaustive: bool,
}

impl<'db> IntoItemId<'db> for Enum<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        self.id(db).into()
    }
}

/// A lowered enum variant and its field layout.
#[salsa::tracked(debug)]
pub struct EnumVariant<'db> {
    #[returns(ref)]
    pub name: String,
    #[tracked]
    #[returns(ref)]
    pub shape: Shape<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

impl<'db> EnumVariant<'db> {
    /// Iterates over this variant's public lowered fields in declaration order.
    pub fn fields(
        self,
        db: &'db dyn crate::codegen::db::Db,
    ) -> impl Iterator<Item = ShapeField<'db>> + Clone + 'db {
        self.shape(db).fields()
    }
}
