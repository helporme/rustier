use std::path::Path;

use super::RustdocId;
use crate::compiler::{
    db::Db,
    ir::doc::RustdocCrates,
    ir::rust::{Crate, Mod, ModParent},
};
use rustdoc_types as doc;

#[salsa::tracked]
impl<'db> Crate<'db> {
    fn data(self, db: &'db dyn Db) -> &'db doc::Crate {
        let rustdoc = RustdocCrates::get(db);
        &rustdoc.crate_data(db, self.name(db)).unwrap().body
    }

    pub(in crate::compiler::ir) fn lower(db: &'db dyn Db, name: &str) -> Option<Self> {
        let rustdoc = RustdocCrates::get(db);

        if rustdoc.crates(db).contains_key(name) {
            Some(Crate::new(db, name))
        } else {
            None
        }
    }

    pub(in crate::compiler::ir) fn lower_dir(self, db: &'db dyn Db) -> &'db Path {
        let rustdoc = RustdocCrates::get(db);
        &rustdoc.crate_data(db, self.name(db)).unwrap().dir
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir) fn lower_crate_root(self, db: &'db dyn Db) -> Mod<'db> {
        let data = self.data(db);

        let id = RustdocId::new(db, self.name(db), data.root);
        Mod::lower(db, id, ModParent::Crate(self))
    }
}
