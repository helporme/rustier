use crate::codegen::{
    db::Db,
    ir::{Fn, IrId, Span},
    rustdoc::{lower_mod_fns, lower_mod_submodules},
};

/// A module definition.
#[salsa::tracked(debug)]
pub struct Mod<'db> {
    pub id: IrId<'db>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

impl<'db> Mod<'db> {
    /// Gets the functions defined in this module.
    pub fn fns(&self, db: &'db dyn Db) -> &[Fn<'db>] {
        lower_mod_fns(db, self.id(db).rustdoc_id(db))
    }

    /// Gets the submodules defined in this module.
    pub fn mods(&self, db: &'db dyn Db) -> &[Mod<'db>] {
        lower_mod_submodules(db, self.id(db).rustdoc_id(db))
    }
}
