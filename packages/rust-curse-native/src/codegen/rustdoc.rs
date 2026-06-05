pub mod lower;

use crate::codegen::db::Db;
use rustdoc_types::{Crate as RCrate, Item as RItem};

pub use rustdoc_types::Id;

#[salsa::input]
pub struct CrateDef {
    #[returns(ref)]
    pub value: RCrate,
}

impl CrateDef {
    pub fn item<'db>(&self, db: &'db dyn Db, id: Id) -> &'db RItem {
        // item should always be found, otherwise how did you get the id?
        self.value(db).index.get(&id).expect("item not found")
    }
}
