use crate::codegen::{
    db::Db,
    ir::shape::{Shape, ShapeField},
    item::ItemId,
    rustdoc::RustdocId,
};

/// Identifies a lowered Rust struct.
#[salsa::interned(debug)]
#[derive(Ord, PartialOrd)]
pub struct StructId<'db> {
    #[returns(copy)]
    pub inner: ItemId<'db>,
}

impl<'db> StructId<'db> {
    /// Returns the fully qualified struct name, e.g. `geometry::Point`.
    pub fn fqn(self, db: &'db dyn Db) -> &'db String {
        self.inner(db).fqn(db)
    }

    /// Returns the unqualified struct name, e.g. `Point`.
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.inner(db).name(db)
    }

    /// Returns the mangled fully qualified struct name, e.g. `geometry_Point`.
    pub fn mangled_fqn(self, db: &'db dyn Db) -> &'db String {
        self.inner(db).mangled_fqn(db)
    }

    /// Returns the [`RustdocId`] matching this struct, or panics if no match is
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

/// A lowered Rust struct and its fields.
#[salsa::tracked(debug)]
pub struct Struct<'db> {
    pub id: StructId<'db>,
    #[tracked]
    #[returns(ref)]
    pub shape: Shape<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

#[salsa::tracked]
impl<'db> ShapeField<'db> {
    /// Returns the struct field binding getter function name, e.g.
    /// `__getter__foo`.
    #[salsa::tracked(returns(ref))]
    pub fn getter_binding_name_on_struct(self, db: &'db dyn Db) -> String {
        format!("__getter__{}", self.name(db))
    }

    /// Returns the struct field binding setter function name, e.g.
    /// `__setter__foo`.
    #[salsa::tracked(returns(ref))]
    pub fn setter_binding_name_on_struct(self, db: &'db dyn Db) -> String {
        format!("__setter__{}", self.name(db))
    }
}

#[salsa::tracked]
impl<'db> Struct<'db> {
    /// Returns the unqualified struct name, e.g. `Point`.
    pub fn name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).name(db)
    }

    /// Returns the fully qualified struct name, e.g. `geometry::Point`.
    pub fn fqn(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).fqn(db)
    }

    /// Returns the public lowered fields in declaration order.
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<ShapeField<'db>> {
        // todo: make .fields() a tracked function instead of this wrapper
        self.shape(db).fields().collect()
    }
}
