use crate::codegen::ir::{
    IrId,
    mangle::to_camel_case,
    shape::{Shape, ShapeField},
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
    /// Returns the enum name, e.g. `Shape` for `geometry::Shape`.
    pub fn name(&self, db: &'db dyn crate::codegen::db::Db) -> &'db str {
        self.id(db).path(db).last().expect("enum must have a name")
    }
}
