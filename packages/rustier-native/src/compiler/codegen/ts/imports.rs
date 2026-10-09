use crate::compiler::{db::Db, ir::decals::DecalImport};

#[salsa::tracked]
impl<'db> DecalImport<'db> {
    /// Local declarations use file paths; external declarations use ambient
    /// modules.
    #[salsa::tracked(returns(ref))]
    pub fn path(self, db: &'db dyn Db) -> String {
        let target = self.class(db).module(db);
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
