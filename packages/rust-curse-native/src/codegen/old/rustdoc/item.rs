use rustdoc_types::ItemEnum;

use crate::codegen::{
    asg,
    db::Db,
    rustdoc::{lower_enum, lower_fn, lower_mod, lower_struct, lower_trait, RustdocId},
};

// todo: id is ustable, replace two params with ItemPtr which will contain
// craete and stable ptr
#[salsa::tracked]
pub fn lower_item<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Option<asg::Item<'db>> {
    let item = id.item(db);
    match &item.inner {
        ItemEnum::Function(..) => Some(asg::Item::Fn(lower_fn(db, id)?)),
        ItemEnum::Struct(..) => Some(asg::Item::Struct(lower_struct(db, id)?)),
        ItemEnum::Trait(..) => Some(asg::Item::Trait(lower_trait(db, id)?)),
        ItemEnum::Enum(..) => Some(asg::Item::Enum(lower_enum(db, id)?)),
        ItemEnum::Module(..) => Some(asg::Item::Mod(lower_mod(db, id))),
        _ => None,
    }
}
