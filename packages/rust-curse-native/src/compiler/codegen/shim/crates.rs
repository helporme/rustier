use crate::compiler::{
    codegen::fragment::{FragmentPrint as _, spaced},
    db::Db,
    ir::shim::ShimCrate,
};

#[salsa::tracked]
impl<'db> ShimCrate<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db) -> String {
        let members = spaced(self.mods(db).iter().flat_map(|mod_| {
            mod_.classes(db)
                .iter()
                .map(|class| class.render(db))
                .chain(mod_.fns(db).iter().map(|fn_| fn_.render(db)))
        }));

        members.print()
    }
}
