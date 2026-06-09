use std::ops::Deref;

use crate::codegen::asg::{ItemPath, Span, StructShape};

/// Path to an enum, including the enum name.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct EnumPath<'db>(pub ItemPath<'db>);

impl<'db> Deref for EnumPath<'db> {
    type Target = ItemPath<'db>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A variant of an enum.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct EnumVariant<'db> {
    pub name: String,
    pub shape: StructShape<'db>,
    pub span: Option<Span>,
    pub docs: Option<String>,
}

/// An enum definition (at least one struct-shaped variant -> data enum).
#[salsa::tracked(debug)]
pub struct EnumDef<'db> {
    pub path: EnumPath<'db>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
    #[tracked]
    #[returns(ref)]
    pub variants: Vec<EnumVariant<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}
