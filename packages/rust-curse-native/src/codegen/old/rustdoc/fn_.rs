use rustdoc_types::{Abi, ItemEnum, Visibility};

use crate::codegen::{
    asg,
    db::Db,
    rustdoc::{lower_path, lower_type, CrateExt, RustdocId},
};

#[salsa::tracked]
pub fn lower_fn<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Option<asg::FnDef<'db>> {
    let item = id.item(db);
    let crate_ = id.crate_(db);

    let def = match &item.inner {
        ItemEnum::Function(def) => def,
        _ => unreachable!(),
    };

    if def.has_body && item.visibility != Visibility::Public {
        return None;
    }

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
        let type_ = lower_type(db, crate_.ty(db, ty));

        if type_.is_none() {
            println!("function is skipped since param {name} is not supported");
            return None;
        }

        params.push(asg::FnParam {
            name: name.clone(),
            ty: type_.unwrap(),
        });
    }

    let ret_type = def
        .sig
        .output
        .as_ref()
        .and_then(|ty| lower_type(db, crate_.ty(db, ty)));

    let path = asg::FnPath(lower_path(db, id));
    let span = item.span.clone().map(|s| s.into());

    Some(asg::FnDef::new(
        db,
        path,
        def.has_body,
        span,
        params,
        ret_type,
        item.docs.clone(),
    ))
}
