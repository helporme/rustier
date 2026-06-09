use crate::codegen::{
    asg,
    db::Db,
    rustdoc::{lower_path, RustdocId},
};

#[salsa::tracked]
pub fn lower_mod<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> asg::ModDef<'db> {
    let item = id.item(db);

    let path = asg::ModPath(lower_path(db, id));
    let span = item.span.clone().map(|s| s.into());

    asg::ModDef::new(db, path, span)
}

// #[salsa::tracked]
// pub fn lower_use(db: &dyn Db, crate_: RustdocCrate, id: Id) ->
// Option<asg::Use<'_>> {     let item = crate_.item(db, id);
//     let def = match &item.inner {
//         ItemEnum::Use(def) => def,
//         _ => unreachable!(),
//     };

//     if def.is_glob {
//         println!("use mod::* are not supported");
//         return None;
//     }

//     let item = lower_item(db, crate_, def.id?)?;

//     Some(asg::Use {
//         name: def.name.clone(),
//         item,
//     })
// }
