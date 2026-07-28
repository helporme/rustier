use crate::codegen::{
    db::Db,
    ir::shape::{Shape, ShapeField},
    item::ItemId,
    rustdoc::RustdocId,
};

#[salsa::interned(debug)]
#[derive(Ord, PartialOrd)]
pub struct EnumId<'db> {
    #[returns(copy)]
    pub inner: ItemId<'db>,
}

impl<'db> EnumId<'db> {
    /// Returns the fully qualified enum name, e.g. `geometry::Shape`.
    pub fn fqn(self, db: &'db dyn Db) -> &'db String {
        self.inner(db).fqn(db)
    }

    /// Returns the unqualified enum name, e.g. `Shape`.
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.inner(db).name(db)
    }

    /// Returns the mangled fully qualified enum name, e.g. `geometry_Shape`.
    pub fn mangled_fqn(self, db: &'db dyn Db) -> &'db String {
        self.inner(db).mangled_fqn(db)
    }

    /// Returns the [`RustdocId`] matching this enum, or panics if no match is
    /// found.
    pub fn rustdoc_id(self, db: &'db dyn Db) -> RustdocId<'db> {
        self.inner(db).rustdoc_id(db)
    }

    /// Returns the parent module ID, or `None` if this ID has no parent path
    /// segment.
    pub fn parent_mod_id(self, db: &'db dyn Db) -> Option<ItemId<'db>> {
        self.inner(db).parent_mod_id(db)
    }
}

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
    pub docs: Option<String>,
}

impl<'db> EnumVariant<'db> {
    /// Iterates over this variant's public lowered fields in declaration order.
    pub fn fields(
        self,
        db: &'db dyn crate::codegen::db::Db,
    ) -> impl Iterator<Item = ShapeField<'db>> + Clone + 'db {
        self.shape(db).fields()
    }
}

/// A lowered Rust enum and its variants.
#[salsa::tracked(debug)]
pub struct Enum<'db> {
    pub id: EnumId<'db>,
    #[tracked]
    #[returns(ref)]
    pub variants: Vec<EnumVariant<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

impl<'db> Enum<'db> {
    /// Returns the unqualified enum name, e.g. `Shape`.
    pub fn name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).name(db)
    }

    /// Returns the fully qualified enum name, e.g. `geometry::Shape`.
    pub fn fqn(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).fqn(db)
    }
}
