use rustdoc_types::{ItemEnum, ItemKind};
use salsa::Accumulator;

use crate::codegen::{db::Db, ir, rustdoc};

/// Returns all rustdoc module IDs available in all crates.
#[salsa::tracked(returns(ref))]
pub fn all_mod_ids<'db>(db: &'db dyn Db) -> Vec<rustdoc::Id<'db>> {
    let rustdoc = rustdoc::Input::get(db);

    let mut mod_ids = Vec::new();

    for (crate_name, crate_) in rustdoc.crates(db) {
        let mut crate_mod_ids = crate_
            .body
            .paths
            .iter()
            .filter(|(id, item_summary)| {
                item_summary.kind == ItemKind::Module && crate_.body.index.contains_key(id)
            })
            .collect::<Vec<_>>();

        crate_mod_ids.sort_by(|(_, a), (_, b)| a.path.cmp(&b.path));

        for (id, _) in crate_mod_ids {
            mod_ids.push(rustdoc::Id::new(db, *id, crate_name));
        }
    }

    mod_ids
}

/// Lowers the rustdoc module.
#[salsa::tracked]
pub fn lower_mod<'db>(db: &'db dyn Db, id: rustdoc::Id<'db>) -> ir::Mod<'db> {
    let item = id.item(db);
    let crate_ = id.crate_body(db);

    let item_summary = crate_.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, ItemKind::Module);
    let span = item.span.clone().map(|span| id.span(db, span));
    let id = ir::Id::new(db, &item_summary.path);

    let docs = item.docs.clone();

    ir::Mod::new(db, id, docs, span)
}

/// Lowers the functions defined inside the given rustdoc module.
#[salsa::tracked(returns(ref))]
pub fn lower_mod_fns<'db>(db: &'db dyn Db, id: rustdoc::Id<'db>) -> Vec<ir::Fn<'db>> {
    let item = id.item(db);
    let crate_ = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Module(def) => def,
        _ => unreachable!(),
    };

    let mut fns = Vec::new();

    for item_id in &def.items {
        let item = crate_.index.get(item_id).unwrap();
        if matches!(item.inner, ItemEnum::Function(_)) {
            match rustdoc::lower_fn(
                db,
                rustdoc::Id::new(db, *item_id, id.crate_name(db).clone()),
            ) {
                Ok(fn_) => fns.push(fn_),
                Err(diags) => {
                    for diag in diags {
                        diag.accumulate(db);
                    }
                }
            }
        }
    }

    fns
}

/// Lowers the submodules defined inside the given rustdoc module.
#[salsa::tracked(returns(ref))]
pub fn lower_mod_submods<'db>(db: &'db dyn Db, id: rustdoc::Id<'db>) -> Vec<ir::Mod<'db>> {
    let item = id.item(db);
    let crate_ = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Module(def) => def,
        _ => unreachable!(),
    };

    let mut mods = Vec::new();

    for item_id in &def.items {
        let item = crate_.index.get(item_id).unwrap();
        if matches!(item.inner, ItemEnum::Module(_)) {
            let submod = lower_mod(
                db,
                rustdoc::Id::new(db, *item_id, id.crate_name(db).clone()),
            );
            mods.push(submod);
        }
    }

    mods
}
