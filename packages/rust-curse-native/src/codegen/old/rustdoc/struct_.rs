use rustdoc_types::{ItemEnum, StructKind};

use crate::codegen::{
    asg,
    db::Db,
    rustdoc::{lower_path, lower_type, CrateExt, RustdocId},
};

#[salsa::tracked]
pub fn lower_struct<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Option<asg::StructDef<'db>> {
    let item = id.item(db);
    let crate_ = id.crate_(db);

    let def = match &item.inner {
        ItemEnum::Struct(def) => def,
        _ => unreachable!(),
    };

    if !def.generics.params.is_empty() {
        println!("generics are unsupported on a struct");
        return None;
    }

    let shape = match &def.kind {
        StructKind::Plain {
            fields,
            has_stripped_fields,
        } => {
            let mut shape_fields = Vec::new();
            let mut has_stripped_fields = *has_stripped_fields;

            for id in fields {
                if let Some(field) = lower_struct_field(db, crate_.id(db, *id)) {
                    shape_fields.push(field);
                } else {
                    has_stripped_fields = true;
                }
            }

            asg::StructShape::Plain {
                fields: shape_fields,
                has_stripped_fields,
            }
        }
        StructKind::Unit => {
            println!("unit structs or enum variants are unsupported");
            return None;
        }
        StructKind::Tuple(..) => {
            println!("tuple structs or enum variants are unsupported");
            return None;
        }
    };

    let path = lower_path(db, id);
    let span = item.span.clone().map(|s| s.into());

    Some(asg::StructDef::new(
        db,
        path,
        span,
        shape,
        item.docs.clone(),
    ))
}

#[salsa::tracked]
pub fn lower_struct_field<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
) -> Option<asg::StructField<'db>> {
    let item = id.item(db);
    let crate_ = id.crate_(db);

    let ty = match &item.inner {
        ItemEnum::StructField(ty) => ty,
        _ => unreachable!(),
    };

    let ty = lower_type(db, crate_.ty(db, ty));

    if ty.is_none() {
        println!("filed is using unsupported type");
        return None;
    }

    Some(asg::StructField {
        name: item.name.clone().unwrap(),
        ty: ty.unwrap(),
        span: item.span.clone().map(|s| s.into()),
        docs: item.docs.clone(),
    })
}
