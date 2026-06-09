//! Lowers the rustdoc function to an ir's [`ir::FnDef`].

use rustdoc_types::{Abi, ItemEnum, ItemKind};

use crate::codegen::{
    db::Db,
    ir,
    rustdoc::{lower_type, CrateExt, LoweredType, RustdocId},
};

/// Lowers the rustdoc function to an ir's [`ir::FnDef`].
#[salsa::tracked]
pub fn lower_fn<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Option<ir::FnDef<'db>> {
    let item = id.item(db);
    let crate_ = id.crate_(db);

    let def = match &item.inner {
        ItemEnum::Function(def) => def,
        _ => unreachable!(),
    };

    assert!(def.has_body, "trait/extern functions cannot be lowerd");

    // todo: warn about all of this at once in case it's used

    if def.header.is_async {
        // todo: warning
        println!("async functions are not supported");
        return None;
    }

    if def.header.is_unsafe {
        // todo: warning
        println!("unsafe functions are not supported");
        return None;
    }

    if def.header.abi != Abi::Rust {
        // todo: warning
        println!("{:?} functions are not supported", def.header.abi);
        return None;
    }

    if def.sig.is_c_variadic {
        // todo: warning
        println!("c-variadic functions are not supported");
        return None;
    }

    if !def.generics.params.is_empty() {
        // todo: warning
        println!("generic functions are not supported");
        return None;
    }

    let mut params = Vec::new();

    for (name, ty) in &def.sig.inputs {
        let lowered_type = lower_type(db, crate_.ty(db, ty));

        if lowered_type.is_none() {
            println!("function is skipped since param {name} is not supported");
            return None;
        }

        let (type_, mode) = match lowered_type.unwrap() {
            LoweredType::Moved(type_) => (type_, ir::ParamMode::Move),
            LoweredType::Borrowed { type_, is_mutable } => (
                type_,
                if is_mutable {
                    ir::ParamMode::Mut
                } else {
                    ir::ParamMode::Ref
                },
            ),
        };

        if matches!(type_, ir::Type::Primitive(_)) && mode == ir::ParamMode::Mut {
            println!("Mutable reference is ignored for primitive types, changes to it won't be reflected in js");
        }

        params.push(ir::FnParam::new(db, name.clone(), type_, mode));
    }

    let ret_type = def
        .sig
        .output
        .as_ref()
        .map(|ty| lower_type(db, crate_.ty(db, ty)));

    let ret_type = match ret_type {
        Some(Some(LoweredType::Moved(type_))) => Some(type_),
        Some(Some(LoweredType::Borrowed { .. })) => {
            println!("you cannot return a borrowed value from the function");
            return None;
        }
        // in case there is return type, but it's not convertible to `ir::Type`
        Some(None) => return None,
        // in case there is no return type
        None => None,
    };

    let item_summary = crate_.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, ItemKind::Function);
    let id = ir::FnId::new(db, &item_summary.path);

    // let path = asg::FnPath(lower_path(db, id));
    let span = item.span.clone().map(|s| s.into());

    Some(ir::FnDef::new(
        db,
        id,
        params,
        ret_type,
        item.docs.clone(),
        span,
    ))
}
