use rustdoc_types::{ItemEnum, ItemKind};

use crate::codegen::{
    db::Db,
    ir::{
        IrId,
        enum_::{Enum, EnumVariant},
    },
    rustdoc::{
        CrateExt, RustdocId,
        shape::{RustdocShape, lower_shape},
    },
};

/// Lowers the rustdoc enum to the IR enum.
#[salsa::tracked]
pub fn lower_enum<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Result<Enum<'db>, ()> {
    let item = id.item(db);
    let crate_body = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Enum(def) => def,
        _ => unreachable!(),
    };

    let item_summary = crate_body.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, ItemKind::Enum);

    if !def.generics.params.is_empty() || def.has_stripped_variants {
        return Err(());
    }

    let variants = def
        .variants
        .iter()
        .map(|variant_id| lower_variant(db, crate_body.id(db, *variant_id)))
        .collect::<Result<Vec<_>, _>>()?;
    let docs = item.docs.clone();
    let id = IrId::new(db, &item_summary.path);

    Ok(Enum::new(db, id, variants, docs))
}

/// Lowers a rustdoc enum variant to an IR enum variant.
fn lower_variant<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Result<EnumVariant<'db>, ()> {
    let item = id.item(db);
    let crate_body = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Variant(def) => def,
        _ => unreachable!(),
    };

    let item_summary = crate_body.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, ItemKind::Variant);

    let (shape, skipped_fields) = lower_shape(db, id, RustdocShape::from(&def.kind));
    if !skipped_fields.is_empty() {
        return Err(());
    }

    let name = item.name.clone().expect("enum variant must have a name");
    let discriminant = def
        .discriminant
        .as_ref()
        .map(|discriminant| discriminant.value.clone());
    let docs = item.docs.clone();

    Ok(EnumVariant::new(db, name, shape, discriminant, docs))
}
