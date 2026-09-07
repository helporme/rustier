use crate::compiler::{
    codegen::{
        fragment::{FragmentPrint, doubles, frag},
        js::JsCodegenPaths,
    },
    db::Db,
    ir::facade::{FacadeImport, FacadeImportItem},
};

#[salsa::tracked]
impl<'db> FacadeImport<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db, paths: JsCodegenPaths) -> String {
        let item = self.item(db);
        let symbol = item.symbol(db);

        frag!(
            import {{ {symbol} }} from {doubles(self.render_path(db, paths))}
        )
        .print()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn render_path(self, db: &'db dyn Db, paths: JsCodegenPaths) -> String {
        let originating_from_path = self.originating_from(db).file_path(db);
        let to_path = match self.item(db) {
            FacadeImportItem::FfiFunction(_) | FacadeImportItem::FfiClass(_) => &paths.ffi_js_file,
            FacadeImportItem::FacadeClass(class) => class.module(db).file_path(db),
        };

        originating_from_path
            .relative_path(to_path)
            .expect("relative path must exist")
    }
}
