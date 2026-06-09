use rustdoc_types::ItemEnum;

use crate::codegen::{
    asg,
    db::Db,
    rustdoc::{lower_path, RustdocId},
};

#[salsa::tracked]
pub fn lower_trait<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Option<asg::TraitDef<'db>> {
    let item = id.item(db);

    let def = match &item.inner {
        ItemEnum::Trait(def) => def,
        _ => unreachable!(),
    };

    if !def.generics.params.is_empty() {
        println!("generics are not supported");
        return None;
    }

    if !def.is_unsafe {
        println!("unsafe trait methods are not supported");
        return None;
    }

    if def.is_auto {
        println!("auto trait methods are not supported");
        return None;
    }

    let path = lower_path(db, id);
    let span = item.span.clone().map(|s| s.into());

    Some(asg::TraitDef::new(db, path, span, item.docs.clone()))
}
