use crate::codegen::ir::{IrId, shape::Shape};

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
