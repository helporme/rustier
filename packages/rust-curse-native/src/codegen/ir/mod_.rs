use crate::codegen::{
    db::Db,
    ir::{Fn, IrId, Span},
    rustdoc::lower_mod_fns,
};

/// A module definition.
#[salsa::tracked(debug)]
pub struct ModDef<'db> {
    pub id: IrId<'db>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

impl<'db> ModDef<'db> {
    /// Returns the functions defined in this module.
    pub fn fns(&self, db: &'db dyn Db) -> &[Fn<'db>] {
        lower_mod_fns(db, self.id(db).rustdoc_id(db))
    }
}
