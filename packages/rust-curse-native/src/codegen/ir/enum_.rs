use crate::codegen::{
    db::Db,
    ir::{
        item::{ItemIdExt as _, ItemKind, define_item_id},
        shape::{Shape, ShapeField},
    },
};

define_item_id!(EnumId, ItemKind::Enum, "Identifies a lowered Rust enum.");

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
}

impl<'db> Enum<'db> {
    /// Returns the unqualified enum name, e.g. `Shape`.
    pub fn name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).name(db)
    }

    /// Returns the fully qualified enum name, e.g. `geometry::Shape`.
    pub fn fqn(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).fqn(db)
    }
}
