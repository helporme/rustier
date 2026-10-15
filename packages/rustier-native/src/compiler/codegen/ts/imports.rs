use crate::compiler::{
    codegen::fragment::{FragmentOnce, frag, singles},
    db::Db,
    ir::facade::imports_ts::{FacadeImportItemTs, FacadeImportTs},
};

#[salsa::tracked]
impl<'db> FacadeImportTs<'db> {
    pub(crate) fn render_ts(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        let item = self.item(db);
        let alias = match item {
            FacadeImportItemTs::Runtime(export) => Some(frag!(~as~{export.import_alias()})),
            FacadeImportItemTs::FacadeClass(_) => None,
        };

        frag!(
            import type ~{{@
                {item.name(db)}{alias}@
            }}~ from ~{singles(self.path(db))};
        )
    }

    /// Local declarations use file paths; external declarations use ambient
    /// modules.
    #[salsa::tracked(returns(ref))]
    pub fn path(self, db: &'db dyn Db) -> String {
        let target = match self.item(db) {
            FacadeImportItemTs::FacadeClass(class) => class.module(db),
            FacadeImportItemTs::Runtime(export) => {
                return export.module().virtual_path().to_owned();
            }
        };
        if target.is_local(db) {
            self.originating_from(db)
                .file_path(db)
                .relative_path(target.file_path(db))
                .expect("relative path must exist")
        } else {
            target.virtual_path(db).to_owned()
        }
    }
}
