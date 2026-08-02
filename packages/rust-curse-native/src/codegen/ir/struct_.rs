use crate::codegen::{
    db::Db,
    ir::{
        item::{IntoItemId, ItemId, ItemKind, define_item_id},
        shape::{Shape, ShapeField},
        span::Span,
    },
};

define_item_id!(
    StructId,
    ItemKind::Struct,
    "Identifies a lowered Rust struct."
);

/// A lowered Rust struct and its fields.
#[salsa::tracked(debug)]
pub struct Struct<'db> {
    pub id: StructId<'db>,
    #[tracked]
    #[returns(ref)]
    pub shape: Shape<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

impl<'db> IntoItemId<'db> for Struct<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        self.id(db).into()
    }
}

#[salsa::tracked]
impl<'db> Struct<'db> {
    /// Returns the public lowered fields in declaration order.
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<ShapeField<'db>> {
        // todo: make .fields() a tracked function instead of this wrapper
        self.shape(db).fields().collect()
    }
}

#[salsa::tracked]
impl<'db> ShapeField<'db> {
    /// Returns the struct field binding getter function name, e.g.
    /// `__getter__foo`.
    #[salsa::tracked(returns(ref))]
    pub fn getter_binding_name_on_struct(self, db: &'db dyn Db) -> String {
        format!("__getter__{}", self.name(db))
    }

    /// Returns the struct field binding setter function name, e.g.
    /// `__setter__foo`.
    #[salsa::tracked(returns(ref))]
    pub fn setter_binding_name_on_struct(self, db: &'db dyn Db) -> String {
        format!("__setter__{}", self.name(db))
    }
}
