use crate::codegen::{
    db::Db,
    ir::{
        item::{IntoItemId, ItemId, ItemKind, define_item_id},
        type_::IrType,
    },
};

define_item_id!(FnId, ItemKind::Fn, "Identifies a lowered Rust function.");

/// A function.
#[salsa::tracked(debug)]
pub struct Fn<'db> {
    pub id: FnId<'db>,
    #[tracked]
    #[returns(ref)]
    pub params: Vec<FnParam<'db>>,
    #[tracked]
    #[returns(copy)]
    pub return_type: Option<IrType<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

impl<'db> IntoItemId<'db> for Fn<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        self.id(db).into()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, salsa::Update)]
pub enum PassingMode {
    Ref,
    Mut,
    Move,
}

/// A function parameter.
#[salsa::tracked(debug)]
pub struct FnParam<'db> {
    #[returns(ref)]
    pub name: String,
    #[tracked]
    pub type_: IrType<'db>,
    #[returns(copy)]
    pub mode: PassingMode,
}
