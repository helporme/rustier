use rustdoc_types::{ItemEnum, ItemKind as RItemKind};

use crate::codegen::{
    db::Db,
    diag::DiagAccumulateExt as _,
    ir::{CLikeEnum, Enum, Fn, Mod, ModId, Struct},
    ir::{Item as IrItem, ItemId, ItemKind, ItemPath},
    rustdoc::{RustdocCrates, RustdocId},
};

use super::{
    enums::{LoweredEnum, lower_enum},
    fns::lower_fn,
    spans::RustdocSpanExt as _,
    structs::lower_struct,
};

/// Returns all rustdoc module IDs available in all crates.
#[salsa::tracked(returns(ref))]
pub fn all_mod_ids<'db>(db: &'db dyn Db) -> Vec<RustdocId<'db>> {
    let rustdoc = RustdocCrates::get(db);

    let mut mod_ids = Vec::new();

    for (crate_name, crate_) in rustdoc.crates(db) {
        let mut crate_mod_ids = crate_
            .body
            .paths
            .iter()
            .filter(|(id, item_summary)| {
                item_summary.kind == RItemKind::Module && crate_.body.index.contains_key(id)
            })
            .collect::<Vec<_>>();

        crate_mod_ids.sort_by(|(_, a), (_, b)| a.path.cmp(&b.path));

        for (id, _) in crate_mod_ids {
            mod_ids.push(RustdocId::new(db, *id, crate_name));
        }
    }

    mod_ids
}

/// Lowers all rustdoc modules in their deterministic path order.
#[salsa::tracked(returns(ref))]
pub fn lower_mods<'db>(db: &'db dyn Db) -> Vec<Mod<'db>> {
    all_mod_ids(db)
        .iter()
        .map(|id| lower_mod(db, *id))
        .collect()
}

/// Lowers the rustdoc module to the IR module.
#[salsa::tracked]
pub fn lower_mod<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Mod<'db> {
    let item = id.item(db);
    let crate_ = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Module(def) => def,
        _ => unreachable!(),
    };
    let item_summary = crate_.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, RItemKind::Module);

    let items = def
        .items
        .iter()
        .filter_map(|item_id| {
            lower_mod_item(db, RustdocId::new(db, *item_id, id.crate_name(db).clone()))
        })
        .collect();

    let docs = item.docs.clone();
    let span = item.span.as_ref().map(|span| span.lower(id.crate_(db)));
    let id = ModId::new(
        db,
        ItemId::new(db, ItemPath::new(db, &item_summary.path), ItemKind::Mod),
    )
    .expect("newly created module item must have module kind");

    Mod::new(db, id, items, docs, span)
}

#[salsa::tracked]
fn lower_mod_item<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Option<IrItem<'db>> {
    match &id.item(db).inner {
        ItemEnum::Function(_) => match lower_fn(db, id) {
            Ok(fn_) => Some(fn_.into()),
            Err(diag) => {
                diag.accumulate(db);
                None
            }
        },
        ItemEnum::Struct(_) => match lower_struct(db, id) {
            Ok(struct_) => Some(struct_.into()),
            Err(diag) => {
                diag.accumulate(db);
                None
            }
        },
        ItemEnum::Enum(_) => match lower_enum(db, id) {
            Ok(LoweredEnum::Enum(enum_)) => Some(enum_.into()),
            Ok(LoweredEnum::CLikeEnum(enum_)) => Some(enum_.into()),
            Err(diag) => {
                diag.accumulate(db);
                None
            }
        },
        ItemEnum::Module(_) => Some(lower_mod(db, id).into()),
        _ => None,
    }
}

/// Lowers the functions defined inside the given rustdoc module to the IR
/// functions.
#[salsa::tracked(returns(ref))]
pub fn lower_mod_fns<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Vec<Fn<'db>> {
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
            match lower_mod_item(db, RustdocId::new(db, *item_id, id.crate_name(db).clone())) {
                Some(IrItem::Fn(fn_)) => fns.push(fn_),
                Some(_) => unreachable!(),
                None => {}
            }
        }
    }

    fns
}

/// Lowers the structs defined inside the given rustdoc module to the IR
/// structs.
#[salsa::tracked(returns(ref))]
pub fn lower_mod_structs<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Vec<Struct<'db>> {
    let item = id.item(db);
    let crate_ = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Module(def) => def,
        _ => unreachable!(),
    };

    let mut structs = Vec::new();

    for item_id in &def.items {
        let item = crate_.index.get(item_id).unwrap();
        if matches!(item.inner, ItemEnum::Struct(_)) {
            match lower_mod_item(db, RustdocId::new(db, *item_id, id.crate_name(db).clone())) {
                Some(IrItem::Struct(struct_)) => structs.push(struct_),
                Some(_) => unreachable!(),
                None => {}
            }
        }
    }

    structs
}

/// Lowers the data-carrying enums defined inside the given rustdoc module.
#[salsa::tracked(returns(ref))]
pub fn lower_mod_enums<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Vec<Enum<'db>> {
    lower_mod_enum_items(db, id)
        .iter()
        .filter_map(|enum_| match enum_ {
            LoweredEnum::Enum(enum_) => Some(*enum_),
            LoweredEnum::CLikeEnum(_) => None,
        })
        .collect()
}

/// Lowers the C-like enums defined inside the given rustdoc module.
#[salsa::tracked(returns(ref))]
pub fn lower_mod_c_like_enums<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Vec<CLikeEnum<'db>> {
    lower_mod_enum_items(db, id)
        .iter()
        .filter_map(|enum_| match enum_ {
            LoweredEnum::Enum(_) => None,
            LoweredEnum::CLikeEnum(enum_) => Some(*enum_),
        })
        .collect()
}

#[salsa::tracked(returns(ref))]
fn lower_mod_enum_items<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Vec<LoweredEnum<'db>> {
    let item = id.item(db);
    let crate_ = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Module(def) => def,
        _ => unreachable!(),
    };

    let mut enums = Vec::new();

    for item_id in &def.items {
        let item = crate_.index.get(item_id).unwrap();
        if matches!(item.inner, ItemEnum::Enum(_)) {
            match lower_mod_item(db, RustdocId::new(db, *item_id, id.crate_name(db).clone())) {
                Some(IrItem::Enum(enum_)) => enums.push(LoweredEnum::Enum(enum_)),
                Some(IrItem::CLikeEnum(enum_)) => enums.push(LoweredEnum::CLikeEnum(enum_)),
                Some(_) => unreachable!(),
                None => {}
            }
        }
    }

    enums
}

/// Lowers the submodules defined inside the given rustdoc module to the IR
/// modules.
#[salsa::tracked(returns(ref))]
pub fn lower_mod_submods<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Vec<Mod<'db>> {
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
            match lower_mod_item(db, RustdocId::new(db, *item_id, id.crate_name(db).clone())) {
                Some(IrItem::Mod(mod_)) => mods.push(mod_),
                Some(_) => unreachable!(),
                None => {}
            }
        }
    }

    mods
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        diag::Diag,
        ir::ItemIdExt as _,
        test::{TEST_CRATE_NAME, test_db},
    };

    #[test]
    fn stores_all_item_kinds() {
        let db = test_db! {
            pub fn first() {}

            pub enum ColorKind {
                Rgb,
                Hsv = 2,
                Cmyk,
            }

            pub struct Point;

            pub enum Value {
                Empty,
                Number(i32),
            }

            pub mod nested {}
        };
        let root = db.rustdoc_id("");

        let fns = lower_mod_fns(&db, root);
        let structs = lower_mod_structs(&db, root);
        let enums = lower_mod_enums(&db, root);
        assert_eq!(enums.len(), 1);
        assert_eq!(
            enums[0].id(&db).fqn(&db),
            &format!("{TEST_CRATE_NAME}::Value")
        );

        let c_like_enums = lower_mod_c_like_enums(&db, root);
        assert_eq!(c_like_enums.len(), 1);
        assert_eq!(
            c_like_enums[0].id(&db).fqn(&db),
            &format!("{TEST_CRATE_NAME}::ColorKind")
        );

        let submods = lower_mod_submods(&db, root);
        let module = lower_mod(&db, root);
        assert_eq!(
            module.items(&db).as_slice(),
            &[
                IrItem::Mod(submods[0]),
                IrItem::Fn(fns[0]),
                IrItem::CLikeEnum(c_like_enums[0]),
                IrItem::Struct(structs[0]),
                IrItem::Enum(enums[0]),
            ]
        );
        assert_eq!(module.fns(&db).collect::<Vec<_>>(), fns.to_vec());
        assert_eq!(module.structs(&db).collect::<Vec<_>>(), structs.to_vec());
        assert_eq!(module.enums(&db).collect::<Vec<_>>(), enums.to_vec());
    }

    #[test]
    fn aggregates_lowering_diagnostics_once() {
        let db = test_db! {
            pub mod nested {
                pub async fn unsupported() {}
            }
        };

        let modules = lower_mods(&db);
        assert!(
            modules
                .iter()
                .all(|module| module.fns(&db).next().is_none())
        );

        let diagnostics = lower_mods::accumulated::<Diag>(&db);
        assert_eq!(diagnostics.len(), 1);
    }
}
