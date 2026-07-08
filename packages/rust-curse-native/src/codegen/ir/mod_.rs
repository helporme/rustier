use crate::codegen::ir::IrId;

/// A module definition.
#[salsa::tracked(debug)]
pub struct Mod<'db> {
    pub id: IrId<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}
