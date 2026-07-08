use crate::codegen::{
    db::Db,
    ir::{Fn, IrId},
    rustdoc::{lower_mod_fns, lower_mod_submods},
};

/// A module definition.
#[salsa::tracked(debug)]
pub struct Mod<'db> {
    pub id: IrId<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

impl<'db> Mod<'db> {
    /// Gets the functions defined in this module.
    pub fn fns(&self, db: &'db dyn Db) -> &[Fn<'db>] {
        lower_mod_fns(db, self.id(db).rustdoc_id(db))
    }

    /// Gets the submodules defined in this module.
    pub fn mods(&self, db: &'db dyn Db) -> &[Mod<'db>] {
        lower_mod_submods(db, self.id(db).rustdoc_id(db))
    }
}
