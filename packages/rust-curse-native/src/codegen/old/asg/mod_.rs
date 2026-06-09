use std::ops::Deref;

use crate::codegen::asg::{Item, ItemPath, Span};

/// Path to a mod, including the mod name.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct ModPath<'db>(pub ItemPath<'db>);

impl<'db> Deref for ModPath<'db> {
    type Target = ItemPath<'db>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A name brought into a module's public surface.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct Use<'db> {
    /// The name this use is given in the module's public surface.
    ///
    /// For example, if this use is `use foo::bar as baz`, then `name` would be
    /// `"baz"`.
    pub name: String,
    /// The item this use refers to.
    pub item: Item<'db>,
}

/// A module definition.
#[salsa::tracked(debug)]
pub struct ModDef<'db> {
    pub path: ModPath<'db>,

    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

// Items brought into this module's public surface.
// #[tracked]
// #[returns(ref)]
// pub uses: Vec<Use<'db>>,

// Public items defined in this module.
// #[tracked]
// #[returns(ref)]
// pub items: Vec<ItemPath<'db>>,
