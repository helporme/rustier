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
        let name = item.name(db);

        frag!(
            import {{ {name} }} from {doubles(self.render_path(db, paths))}
        )
        .print()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn render_path(self, db: &'db dyn Db, paths: JsCodegenPaths) -> String {
        let to_path = match self.item(db) {
            FacadeImportItem::FfiFunction(_) | FacadeImportItem::FfiClass(_) => {
                // Virtual facades have no filesystem directory to resolve against.
                return paths.ffi_js_file.as_str().to_owned();
            }
            FacadeImportItem::FacadeClass(class) => {
                let target = class.module(db);
                if !target.is_local(db) {
                    // We assume here that vite (rolldown) is used which can resolve such virtual
                    // paths, or other bundler that supports virtual paths.
                    return target.virtual_path(db).to_owned();
                }
                target.file_path(db)
            }
        };

        if !self.originating_from(db).as_parent(db).is_local(db) {
            return to_path.as_str().to_owned();
        }

        self.originating_from(db)
            .file_path(db)
            .relative_path(to_path)
            .expect("relative path must exist")
    }
}
