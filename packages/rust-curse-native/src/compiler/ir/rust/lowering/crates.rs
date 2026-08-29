use super::vfs_path;
use crate::compiler::{
    db::Db,
    inputs::{CrateApiDiscovery, CratesMetadata},
    ir::rust::{Crate, Mod},
    path::FilePath,
};

#[salsa::tracked]
impl<'db> Crate<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_name(self, db: &'db dyn Db) -> String {
        self.hir(db)
            .display_name(db)
            .map(|name| name.to_string())
            .unwrap_or_default()
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_crate_root(self, db: &'db dyn Db) -> Mod<'db> {
        Mod::new(db, self.hir(db).root_module(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_dir(self, db: &'db dyn Db) -> FilePath {
        vfs_path(db, self.hir(db).root_file(db))
            .and_then(|path| path.parent())
            .map(FilePath::from)
            .expect("crate root must belong to a workspace source root")
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_api_discovery(
        self,
        db: &'db dyn Db,
    ) -> CrateApiDiscovery {
        CratesMetadata::get(db)
            .crates
            .get(self.root_file(db))
            .expect("lowered crates must have compiler metadata")
            .api_discovery
    }
}
