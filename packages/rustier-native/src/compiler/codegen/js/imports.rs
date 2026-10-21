use crate::compiler::{
    codegen::{
        fragment::{FragmentOnce, FragmentPrint, doubles, frag},
        js::JsCodegenPaths,
    },
    db::Db,
    ir::facade::imports_js::{FacadeImportItemJs, FacadeImportJs},
};

#[salsa::tracked]
impl<'db> FacadeImportJs<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db, paths: JsCodegenPaths) -> String {
        self.item(db)
            .render_js_from(db, self.render_path(db, paths))
            .print()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn render_path(self, db: &'db dyn Db, paths: JsCodegenPaths) -> String {
        let to_path = match self.item(db) {
            FacadeImportItemJs::TransportFunction(_) => {
                return paths.abi_js_mod_id.as_str().to_owned();
            }
            FacadeImportItemJs::Runtime(export) => {
                return export.module().virtual_path().to_owned();
            }
            FacadeImportItemJs::FacadeClass(class) => {
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

impl<'db> FacadeImportItemJs<'db> {
    pub(crate) fn render_js_from(self, db: &'db dyn Db, path: &'db str) -> impl FragmentOnce + 'db {
        let name = self.name(db);
        let alias = match self {
            Self::FacadeClass(class) => Some(class.import_alias(db).as_str()),
            Self::Runtime(export) => Some(export.import_alias()),
            Self::TransportFunction(_) => None,
        }
        .map(|alias| frag!(~as~{alias}));

        frag!(
            import {{ {name}{alias} }} from {doubles(path)}
        )
    }
}
