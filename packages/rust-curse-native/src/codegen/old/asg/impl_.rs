use crate::codegen::asg::{EnumDef, StructDef, TraitDef};

/// Target item for an `impl` definition.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ImplFor<'db> {
    Struct(StructDef<'db>),
    Enum(EnumDef<'db>),
}

/// An `impl` definition.
#[salsa::tracked(debug)]
pub struct ImplDef<'db> {
    /// Struct or enum this impl is for.
    pub for_: ImplFor<'db>,
    /// `None` for an inherent impl, path to the trait this impl implements
    /// otherwise.
    pub trait_: Option<TraitDef<'db>>,
}

// Associated fns this impl defines.
// #[tracked]
// #[returns(ref)]
// pub fns: Vec<FnPath<'db>>,
