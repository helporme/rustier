use crate::codegen::{
    asg::{FnDef, ItemPath, Span},
    db::Db,
    rustdoc::Rustdoc,
};

/// A `trait` definition.
#[salsa::tracked(debug)]
pub struct TraitDef<'db> {
    pub path: ItemPath<'db>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

// #[salsa::tracked]
// impl TraitDef<'_> {
//     #[salsa::tracked]
//     pub fn fns<'db>(&self, db: &'db dyn Db) -> Vec<FnDef<'db>> {
//         Rustdoc::get(db);
//         // path.

//         todo!()
//     }
// }

#[salsa::tracked]
pub fn trait_fns<'db>(db: &'db dyn Db, def: TraitDef<'db>) -> Vec<FnDef<'db>> {
    let rustdoc = Rustdoc::get(db);
    let path = def.path(db);
    let crate_name = path.crate_name(db);
    let crate_ = rustdoc.crate_(db, crate_name);

    todo!()
}
