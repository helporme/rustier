use rustdoc_types::ItemEnum;

use crate::codegen::{
    asg,
    db::Db,
    rustdoc::{trait_::lower_trait, type_::lower_type, CrateExt, RustdocId},
};

#[salsa::tracked]
pub fn lower_impl<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Option<asg::ImplDef<'db>> {
    let item = id.item(db);
    let crate_ = id.crate_(db);

    let def = match &item.inner {
        ItemEnum::Impl(def) => def,
        _ => unreachable!(),
    };

    if def.is_synthetic | def.is_negative {
        return None;
    }

    if !def.generics.params.is_empty() {
        println!("generics are not supported");
        return None;
    }

    if def.is_unsafe {
        println!("unsafe impls are not supported");
        return None;
    }

    if !def.provided_trait_methods.is_empty() {
        println!("provided trait methods are not supported");
        // we are fine with ignoring them
    }

    let for_ = lower_type(db, crate_.ty(db, &def.for_));

    let for_ = match for_ {
        None => {
            println!("skipped because of for_");
            return None;
        }
        Some(asg::Type::Enum(for_)) => asg::ImplFor::Enum(for_),
        Some(asg::Type::Struct(for_)) => asg::ImplFor::Struct(for_),
        _ => unreachable!(),
    };

    let trait_ = def
        .trait_
        .as_ref()
        .and_then(|trait_path| lower_trait(db, crate_.id(db, trait_path.id)));

    Some(asg::ImplDef::new(db, for_, trait_))
}
