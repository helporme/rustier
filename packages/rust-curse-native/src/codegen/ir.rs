use std::collections::HashMap;

pub mod fn_;
pub mod mangle;
pub mod mod_;
pub mod span;
pub mod struct_;
pub mod type_;

use crate::codegen::{
    db::Db,
    rustdoc::{RustdocCrates, RustdocId},
};

#[salsa::interned(debug)]
#[derive(Ord, PartialOrd)]
pub struct IrId<'db> {
    /// Fully qualified path to the item, e.g. [`std`, `mem`, `drop`]
    #[returns(ref)]
    pub path: Vec<String>,
}

#[salsa::tracked]
impl<'db> IrId<'db> {
    /// Returns the fully qualified path to the item, e.g. `std::mem::drop`.
    #[salsa::tracked(returns(ref))]
    pub fn fqn(self, db: &'db dyn Db) -> String {
        self.path(db).join("::")
    }

    /// Returns the [`RustdocId`] matching this item, or panics if no match is
    /// found.
    #[salsa::tracked]
    pub fn rustdoc_id(self, db: &'db dyn Db) -> RustdocId<'db> {
        let backmap = rustdoc_backmap(db);

        // todo: remove panic
        *backmap
            .get(&self)
            .unwrap_or_else(|| panic!("no rustdoc item for `{}`", self.path(db).join("::")))
    }

    /// Returns the parent module ID of this item, or `None` if the item
    /// is a top-level module.
    #[salsa::tracked]
    pub fn get_parent_mod_id(self, db: &'db dyn Db) -> Option<IrId<'db>> {
        let path = self.path(db);
        if path.len() <= 1 {
            return None;
        }

        // todo: result of this function invokation is always unwrapped since it's used
        // in a context where it's obvious that the function returns Some (like id of
        // fn, struct etc.). It might be worth considering adding a special ids like
        // FnId to avoid it.
        Some(IrId::new(db, path[..path.len() - 1].to_vec()))
    }
}

#[salsa::tracked(returns(ref))]
fn rustdoc_backmap<'db>(db: &'db dyn Db) -> HashMap<IrId<'db>, RustdocId<'db>> {
    let rustdoc = RustdocCrates::get(db);

    let mut backmap = HashMap::new();

    for (crate_name, crate_) in rustdoc.crates(db) {
        for (idx, item_summary) in &crate_.body.paths {
            if !crate_.body.index.contains_key(idx) {
                continue;
            }

            let id = IrId::new(db, &item_summary.path);
            backmap.insert(id, RustdocId::new(db, *idx, crate_name));
        }
    }

    backmap
}
