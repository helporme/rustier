use crate::codegen::asg::{EnumDef, FnDef, ModDef, StructDef, TraitDef};

/// Complete path to an item as it appears in the source crate,
/// e.g. `crate_name::module::Type` or a
/// `external_crate::module::Type` for external crate.
#[salsa::interned(debug)]
pub struct ItemPath<'db> {
    #[returns(ref)]
    /// Name of the crate this path belongs to, e.g. `alloc`.
    pub crate_name: String,

    /// Fully qualified path to the item, e.g. `alloc::vec::Vec<String>`.
    #[returns(ref)]
    pub path: String,
}

/// Anything that can be defined in the module.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Item<'db> {
    Mod(ModDef<'db>),
    Struct(StructDef<'db>),
    Enum(EnumDef<'db>),
    Trait(TraitDef<'db>),
    Fn(FnDef<'db>),
}
