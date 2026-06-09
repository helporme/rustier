use crate::codegen::asg::{ItemPath, Span, TraitDef, Type};

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct FnParam<'db> {
    pub name: String,
    pub ty: Type<'db>,
}

#[salsa::tracked(debug)]
pub struct FnDef<'db> {
    pub path: ItemPath<'db>,
    #[tracked]
    pub trait_: Option<TraitDef<'db>>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
    #[tracked]
    #[returns(ref)]
    pub params: Vec<FnParam<'db>>,
    #[tracked]
    #[returns(ref)]
    pub return_type: Option<Type<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}
