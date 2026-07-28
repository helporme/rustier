use rustdoc_types::ItemEnum;

use crate::codegen::{
    db::Db,
    ir::item::ItemId,
    ir::type_::Type,
    rustdoc::{
        CrateExt, RustdocId,
        shape::{RustdocShape, lower_shape},
    },
};

/// A field declared by a struct or enum variant.
#[salsa::tracked(debug)]
pub struct ShapeField<'db> {
    #[returns(ref)]
    pub name: String,
    #[tracked]
    pub type_: Type<'db>,
    #[tracked]
    pub is_readonly: bool,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

#[salsa::tracked]
impl<'db> ShapeField<'db> {
    /// Returns the rustdoc ID of this field under the given parent struct or
    /// enum.
    #[salsa::tracked(returns(copy))]
    pub fn rustdoc_id(self, db: &'db dyn Db, parent_id: ItemId<'db>) -> RustdocId<'db> {
        let parent_rustdoc_id = parent_id.rustdoc_id(db);
        let parent_item = parent_rustdoc_id.item(db);
        let crate_body = parent_rustdoc_id.crate_body(db);

        let field_id = match &parent_item.inner {
            ItemEnum::Struct(def) => {
                rustdoc_id_from_shape(db, parent_rustdoc_id, RustdocShape::from(&def.kind), self)
            }
            ItemEnum::Enum(def) => def.variants.iter().find_map(|variant_id| {
                let variant_id = crate_body.id(db, *variant_id);
                let variant = match &variant_id.item(db).inner {
                    ItemEnum::Variant(variant) => variant,
                    _ => unreachable!(),
                };

                rustdoc_id_from_shape(db, variant_id, RustdocShape::from(&variant.kind), self)
            }),
            _ => panic!(
                "expected struct or enum when getting rustdoc item for `{}`",
                parent_id.fqn(db)
            ),
        };

        field_id.expect("field must be found under its parent item")
    }
}

fn rustdoc_id_from_shape<'db>(
    db: &'db dyn Db,
    shape_id: RustdocId<'db>,
    rustdoc_shape: RustdocShape<'_>,
    field: ShapeField<'db>,
) -> Option<RustdocId<'db>> {
    let crate_body = shape_id.crate_body(db);

    match rustdoc_shape {
        RustdocShape::Unit => None,
        RustdocShape::Tuple(field_ids) => {
            let (Shape::Tuple(fields), _) =
                lower_shape(db, shape_id, RustdocShape::Tuple(field_ids))
            else {
                unreachable!()
            };

            fields
                .iter()
                .zip(field_ids)
                .find_map(|(lowered_field, field_id)| {
                    if lowered_field.as_ref() == Some(&field) {
                        Some(crate_body.id(db, *field_id.as_ref()?))
                    } else {
                        None
                    }
                })
        }
        RustdocShape::Plain { fields, .. } => fields.iter().find_map(|field_id| {
            let rustdoc_shape = RustdocShape::Plain {
                fields: std::slice::from_ref(field_id),
                has_stripped_fields: false,
            };
            let (Shape::Plain { fields, .. }, _) = lower_shape(db, shape_id, rustdoc_shape) else {
                unreachable!()
            };

            fields
                .contains(&field)
                .then(|| crate_body.id(db, *field_id))
        }),
    }
}

/// The field layout shared by structs and enum variants.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum Shape<'db> {
    Unit,
    /// Tuple fields, where private or hidden fields are represented by `None`.
    Tuple(Vec<Option<ShapeField<'db>>>),
    Plain {
        fields: Vec<ShapeField<'db>>,
        has_stripped_fields: bool,
    },
}

impl<'db> Shape<'db> {
    /// Iterates over the public lowered fields in their declaration order.
    pub fn fields(&self) -> impl Iterator<Item = ShapeField<'db>> + Clone + '_ {
        let (tuple_fields, plain_fields): (&[Option<ShapeField<'db>>], &[ShapeField<'db>]) =
            match self {
                Self::Unit => (&[], &[]),
                Self::Tuple(fields) => (fields, &[]),
                Self::Plain { fields, .. } => (&[], fields),
            };

        tuple_fields.iter().flatten().chain(plain_fields).copied()
    }
}
