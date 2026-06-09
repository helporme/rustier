use rustdoc_types::{ItemEnum, ItemKind};

use crate::codegen::{
    db::Db,
    ir,
    rustdoc::{lower_fn, RustdocId},
};

/// Lowers the rustdoc module to an [`ir::ModDef`].
#[salsa::tracked]
pub fn lower_mod<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> ir::ModDef<'db> {
    let item = id.item(db);
    let crate_ = id.crate_(db);

    let item_summary = crate_.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, ItemKind::Module);
    let id = ir::IrId::new(db, &item_summary.path);

    let span = item.span.clone().map(|s| s.into());

    ir::ModDef::new(db, id, span)
}

/// Lowers the rustdoc module functions to [`ir::FnDef`]s.
#[salsa::tracked(returns(ref))]
pub fn lower_mod_fns<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Vec<ir::Fn<'db>> {
    let item = id.item(db);
    let crate_ = id.crate_(db);

    let def = match &item.inner {
        ItemEnum::Module(def) => def,
        _ => unreachable!(),
    };

    let mut fns = Vec::new();

    for item_id in &def.items {
        let item = crate_.index.get(item_id).unwrap();
        if matches!(item.inner, ItemEnum::Function(_)) {
            if let Some(fn_) = lower_fn(db, RustdocId::new(db, *item_id, id.crate_name(db).clone()))
            {
                fns.push(fn_);
            }
        }
    }

    fns
}
