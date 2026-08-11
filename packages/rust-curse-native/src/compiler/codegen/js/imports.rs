use crate::{
    compiler::{
        codegen::fragment::{FragmentOnce, doubles},
        db::Db,
        ir::facade::FacadeImport,
    },
    frag,
};

#[salsa::tracked]
impl<'db> FacadeImport<'db> {
    pub(super) fn render(self, db: &'db dyn Db) -> impl FragmentOnce {
        let item = self.item(db);
        let symbol = item.symbol(db);

        frag!(
            import {{ {symbol} }} from {doubles(self.path(db))}
        )
    }
}
