use rustdoc_types as doc;

use super::{
    ItemExt as _, RustdocId,
    fields::{FieldLowerError, lower_field},
};
use crate::compiler::{
    db::Db,
    ir::rust::{FieldParent, Shape},
};

pub enum RustdocShape<'a> {
    Unit,
    Tuple(&'a [Option<doc::Id>]),
    Plain {
        fields: &'a [doc::Id],
        has_stripped_fields: bool,
    },
}

impl<'db> Shape<'db> {
    pub(super) fn lower(
        db: &'db dyn Db,
        shape: RustdocShape<'_>,
        parent: FieldParent<'db>,
    ) -> (Self, Vec<FieldLowerError>) {
        let item_id = match parent {
            FieldParent::Struct(struct_) => struct_.rustdoc_id(db),
            FieldParent::Variant(variant) => variant.rustdoc_id(db),
        };

        match shape {
            RustdocShape::Unit => (Self::Unit, vec![]),
            RustdocShape::Tuple(ids) => {
                let (fields, errors): (Vec<_>, Vec<_>) = ids
                    .iter()
                    .map(|id| {
                        id.as_ref().map(|id| {
                            lower_field(
                                db,
                                RustdocId::new(db, item_id.crate_name(db), id),
                                parent,
                                None,
                            )
                        })
                    })
                    .fold((vec![], vec![]), |(mut fields, mut errors), result| {
                        match result {
                            Some(Ok(field)) => fields.push(Some(field)),
                            Some(Err(error)) => {
                                errors.push(*error);
                                fields.push(None);
                            }
                            None => fields.push(None),
                        }
                        (fields, errors)
                    });

                (Self::Tuple(fields), errors)
            }
            RustdocShape::Plain {
                fields,
                has_stripped_fields,
            } => {
                let (fields, errors): (Vec<_>, Vec<_>) = fields
                    .iter()
                    .map(|id| {
                        lower_field(
                            db,
                            RustdocId::new(db, item_id.crate_name(db), id),
                            parent,
                            None,
                        )
                    })
                    .fold((vec![], vec![]), |(mut fields, mut errors), result| {
                        match result {
                            Ok(field) => fields.push(field),
                            Err(error) => errors.push(*error),
                        }
                        (fields, errors)
                    });

                let has_stripped_fields = has_stripped_fields || !errors.is_empty();

                (
                    Self::Plain {
                        fields,
                        has_stripped_fields,
                    },
                    errors,
                )
            }
        }
    }
}
