use rustdoc_types::{ItemEnum, ItemKind};

use crate::codegen::{
    db::Db,
    diag::DiagAccumulateExt as _,
    ir::{IrId, c_like_enum::CLikeEnum, enum_::Enum, fn_::Fn, mod_::Mod, struct_::Struct},
    rustdoc::{
        RustdocCrates, RustdocId,
        enum_::{LoweredEnum, lower_enum},
        fn_::lower_fn,
        struct_::lower_struct,
    },
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
                item_summary.kind == ItemKind::Module && crate_.body.index.contains_key(id)
            })
            .collect::<Vec<_>>();

        crate_mod_ids.sort_by(|(_, a), (_, b)| a.path.cmp(&b.path));

        for (id, _) in crate_mod_ids {
            mod_ids.push(RustdocId::new(db, *id, crate_name));
        }
    }

    mod_ids
}

/// Lowers the rustdoc module to the IR module.
#[salsa::tracked]
pub fn lower_mod<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Mod<'db> {
    let item = id.item(db);
    let crate_ = id.crate_body(db);

    let item_summary = crate_.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, ItemKind::Module);
    let id = IrId::new(db, &item_summary.path);

    let docs = item.docs.clone();

    Mod::new(db, id, docs)
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
            match lower_fn(db, RustdocId::new(db, *item_id, id.crate_name(db).clone())) {
                Ok(fn_) => fns.push(fn_),
                Err(diag) => {
                    diag.accumulate(db);
                }
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
            match lower_struct(db, RustdocId::new(db, *item_id, id.crate_name(db).clone())) {
                Ok(struct_) => structs.push(struct_),
                Err(diag) => {
                    diag.accumulate(db);
                }
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
            match lower_enum(db, RustdocId::new(db, *item_id, id.crate_name(db).clone())) {
                Ok(enum_) => enums.push(enum_),
                Err(diag) => {
                    diag.accumulate(db);
                }
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
            let submod = lower_mod(db, RustdocId::new(db, *item_id, id.crate_name(db).clone()));
            mods.push(submod);
        }
    }

    mods
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::test::{TEST_CRATE_NAME, test_db};

    #[test]
    fn separates_data_carrying_and_c_like_enums() {
        let db = test_db! {
            pub enum ColorKind {
                Rgb,
                Hsv = 2,
                Cmyk,
            }

            pub enum Value {
                Empty,
                Number(i32),
            }
        };
        let root = db.rustdoc_id("");

        let enums = lower_mod_enums(&db, root);
        assert_eq!(enums.len(), 1);
        assert_eq!(enums[0].id(&db).path(&db), &[TEST_CRATE_NAME, "Value"]);

        let c_like_enums = lower_mod_c_like_enums(&db, root);
        assert_eq!(c_like_enums.len(), 1);
        assert_eq!(
            c_like_enums[0].id(&db).path(&db),
            &[TEST_CRATE_NAME, "ColorKind"]
        );
    }
}
