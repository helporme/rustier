mod fn_;
mod mangle;
mod mod_;
mod span;
mod type_;

use std::collections::HashMap;

pub use fn_::*;
pub use mod_::*;
pub use span::*;
pub use type_::*;

use crate::codegen::{
    db::Db,
    rustdoc::{RustdocId, RustdocInput},
};

#[salsa::interned(debug)]
pub struct IrId<'db> {
    /// Fully qualified path to the item, e.g. [`std`, `mem`, `drop`]
    #[returns(ref)]
    pub path: Vec<String>,
}

#[salsa::tracked]
fn id_to_rustdoc_id<'db>(db: &'db dyn Db, id: IrId<'db>) -> RustdocId<'db> {
    let backmap = rustdoc_backmap(db);

    *backmap
        .get(&id)
        .unwrap_or_else(|| panic!("no rustdoc item for `{}`", id.path(db).join("::")))
}

#[salsa::tracked(returns(ref))]
fn rustdoc_backmap<'db>(db: &'db dyn Db) -> HashMap<IrId<'db>, RustdocId<'db>> {
    let rustdoc = RustdocInput::get(db);

    let mut backmap = HashMap::new();

    for (crate_name, crate_) in rustdoc.crates(db) {
        for (idx, item_summary) in &crate_.paths {
            if !crate_.index.contains_key(idx) {
                continue;
            }

            let id = IrId::new(db, &item_summary.path);
            backmap.insert(id, RustdocId::new(db, *idx, crate_name));
        }
    }

    backmap
}

impl<'db> IrId<'db> {
    pub fn rustdoc_id(&self, db: &'db dyn Db) -> RustdocId<'db> {
        id_to_rustdoc_id(db, *self)
    }
}
