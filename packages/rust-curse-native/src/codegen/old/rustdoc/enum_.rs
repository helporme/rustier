use rustdoc_types::{ItemEnum, VariantKind};

use crate::codegen::{
    asg,
    db::Db,
    rustdoc::{lower_path, lower_struct_field, CrateExt, RustdocId},
};

#[salsa::tracked]
pub fn lower_enum<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Option<asg::EnumDef<'db>> {
    let item = id.item(db);
    let crate_ = id.crate_(db);

    let def = match &item.inner {
        ItemEnum::Enum(def) => def,
        _ => unreachable!(),
    };

    if def.has_stripped_variants {
        println!("enums with stripped variants are not supported");
        return None;
    }

    if !def.generics.params.is_empty() {
        println!("generics are not supported");
        return None;
    }

    // let mut impls = Vec::new();
    // for item in &def.impls {
    //     if let Some(impl_) = lower_impl(db, crate_, *item) {
    //         impls.push(impl_);
    //     }
    // }

    let mut variants = Vec::new();
    for variant_id in &def.variants {
        if let Some(variant) = lower_enum_variant(db, crate_.id(db, *variant_id)) {
            variants.push(variant);
        } else {
            println!("failed to lower enum variant");
            return None;
        }
    }

    let path = asg::EnumPath(lower_path(db, id));
    let span = item.span.clone().map(|s| s.into());

    Some(asg::EnumDef::new(
        db,
        path,
        span,
        variants,
        item.docs.clone(),
    ))
}

fn lower_enum_variant<'db>(db: &'db dyn Db, id: RustdocId) -> Option<asg::EnumVariant<'db>> {
    let item = id.item(db);
    let crate_ = id.crate_(db);

    let def = match &item.inner {
        ItemEnum::Variant(def) => def,
        _ => unreachable!(),
    };

    if def.discriminant.is_some() {
        println!("variants with discriminants are not supported");
        return None;
    }

    match &def.kind {
        VariantKind::Plain => {
            println!("plain variants are not supported");
            None
        }
        VariantKind::Tuple(..) => {
            println!("tuple variants are not supported");
            None
        }
        VariantKind::Struct {
            fields,
            has_stripped_fields,
        } => {
            let mut has_stripped_fields = *has_stripped_fields;
            let mut lowered_fields = Vec::new();

            for id in fields {
                if let Some(field) = lower_struct_field(db, crate_.id(db, *id)) {
                    lowered_fields.push(field);
                } else {
                    has_stripped_fields = true;
                }
            }

            let shape = asg::StructShape::Plain {
                fields: lowered_fields,
                has_stripped_fields,
            };

            Some(asg::EnumVariant {
                name: item.name.clone().expect("variant name is missing"),
                shape,
                span: item.span.clone().map(|s| s.into()),
                docs: item.docs.clone(),
            })
        }
    }
}
