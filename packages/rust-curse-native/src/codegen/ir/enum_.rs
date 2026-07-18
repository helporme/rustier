use rustdoc_types::ItemKind;

use crate::codegen::{
    db::Db,
    ir::{
        IrId,
        mangle::{mangle_path, to_camel_case},
        shape::{Shape, ShapeField},
        type_::Type,
    },
};

/// A lowered enum variant and its field layout.
#[salsa::tracked(debug)]
pub struct EnumVariant<'db> {
    #[returns(ref)]
    pub name: String,
    #[tracked]
    #[returns(ref)]
    pub shape: Shape<'db>,
    #[tracked]
    #[returns(ref)]
    /// The discriminant value of this variant as number between [`i128::MIN`]
    /// and [`u128::MAX`], if any.
    pub discriminant: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

#[salsa::tracked]
impl<'db> EnumVariant<'db> {
    /// Returns the lower-camel-case tag used to discriminate this variant in
    /// JavaScript.
    #[salsa::tracked(returns(ref))]
    pub fn js_tag_name(self, db: &'db dyn crate::codegen::db::Db) -> String {
        to_camel_case(self.name(db))
    }

    /// Returns all fields declared by this variant.
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn crate::codegen::db::Db) -> Vec<ShapeField<'db>> {
        match self.shape(db) {
            Shape::Unit => vec![],
            Shape::Tuple(fields) => fields.iter().flatten().copied().collect(),
            Shape::Plain { fields, .. } => fields.clone(),
        }
    }
}

/// A lowered Rust enum and its variants.
#[salsa::tracked(debug)]
pub struct Enum<'db> {
    pub id: IrId<'db>,
    #[tracked]
    #[returns(ref)]
    pub variants: Vec<EnumVariant<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

impl<'db> Enum<'db> {
    /// Returns the name of the binding struct for this enum.
    pub fn binding_name(&self, db: &'db dyn Db) -> &'db str {
        enum_binding_name_from_ir_id(db, self.id(db))
    }

    /// Returns the name of the tethered backing struct for this enum.
    pub fn tethered_name(&self, db: &'db dyn Db) -> &'db str {
        enum_tethered_name_from_ir_id(db, self.id(db))
    }

    /// Returns the enum name, e.g. `Shape` for `geometry::Shape`.
    pub fn name(&self, db: &'db dyn Db) -> &'db str {
        enum_name_from_ir_id(db, self.id(db))
    }

    /// Returns the fully qualified enum name, e.g. `geometry::Shape`.
    pub fn fqn(&self, db: &'db dyn Db) -> &'db str {
        enum_fqn_from_ir_id(db, self.id(db))
    }

    /// Returns the enum as a [`Type`].
    pub fn as_type(&self, db: &'db dyn Db) -> Type<'db> {
        Type::Enum(self.id(db))
    }
}

/// Returns the mangled enum name for an IR ID.
#[salsa::tracked(returns(ref))]
pub fn enum_mangled_name_from_ir_id<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    let id = id.rustdoc_id(db);
    let summary = id
        .crate_body(db)
        .paths
        .get(&id.idx(db))
        .expect("Enum not found in a rustdoc crate");

    assert_eq!(summary.kind, ItemKind::Enum);
    mangle_path(&summary.path)
}

/// Returns the Rust binding struct name for an enum IR ID.
#[salsa::tracked(returns(ref))]
pub fn enum_binding_name_from_ir_id<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    format!("__cursed_struct__{}", enum_mangled_name_from_ir_id(db, id))
}

/// Returns the tethered backing struct name for an enum IR ID.
#[salsa::tracked(returns(ref))]
pub fn enum_tethered_name_from_ir_id<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    format!(
        "__tethered_struct__{}",
        enum_mangled_name_from_ir_id(db, id)
    )
}

/// Returns the fully qualified enum name for an IR ID.
#[salsa::tracked(returns(ref))]
pub fn enum_fqn_from_ir_id<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    let id = id.rustdoc_id(db);
    let summary = id
        .crate_body(db)
        .paths
        .get(&id.idx(db))
        .expect("Enum not found in a rustdoc crate");

    assert_eq!(summary.kind, ItemKind::Enum);
    summary.path.join("::")
}

/// Returns the unqualified enum name for an IR ID.
#[salsa::tracked(returns(ref))]
pub fn enum_name_from_ir_id<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    let id = id.rustdoc_id(db);
    let summary = id
        .crate_body(db)
        .paths
        .get(&id.idx(db))
        .expect("Enum not found in a rustdoc crate");

    assert_eq!(summary.kind, ItemKind::Enum);
    summary
        .path
        .last()
        .expect("Enum must have a name")
        .to_string()
}
