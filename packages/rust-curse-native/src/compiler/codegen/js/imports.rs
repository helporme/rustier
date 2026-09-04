use crate::compiler::{
    codegen::fragment::{FragmentPrint, doubles, frag},
    db::Db,
    ir::facade::FacadeImport,
};

#[salsa::tracked]
impl<'db> FacadeImport<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db) -> String {
        let item = self.item(db);
        let symbol = item.symbol(db);

        frag!(
            import {{ {symbol} }} from {doubles(self.from_path(db))}
        )
        .print()
    }
}
