use crate::codegen::ir::IrId;

/// A lowered fieldless enum variant and its optional explicit discriminant.
#[salsa::tracked(debug)]
pub struct CLikeEnumVariant<'db> {
    #[returns(ref)]
    pub name: String,
    /// The discriminant value of this variant as a number between [`i128::MIN`]
    /// and [`u128::MAX`], if any.
    #[tracked]
    #[returns(ref)]
    pub discriminant: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

/// A lowered Rust enum that contains only unit variants.
#[salsa::tracked(debug)]
pub struct CLikeEnum<'db> {
    pub id: IrId<'db>,
    #[tracked]
    #[returns(ref)]
    pub variants: Vec<CLikeEnumVariant<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}
