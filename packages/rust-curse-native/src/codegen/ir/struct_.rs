use rustdoc_types::ItemKind;

use crate::codegen::{
    db::Db,
    ir::{
        IrId,
        mangle::mangle_path,
        shape::{Shape, ShapeField},
        type_::Type,
    },
    rustdoc::RustdocId,
};

#[salsa::interned(debug)]
#[derive(Ord, PartialOrd)]
pub struct StructId<'db> {
    #[returns(copy)]
    pub inner: IrId<'db>,
}

#[salsa::tracked]
impl<'db> StructId<'db> {
    /// Struct fully qualified name, e.g. `std::mem::Drop`.
    #[salsa::tracked(returns(ref))]
    pub fn fqn(self, db: &'db dyn Db) -> String {
        let id = self.rustdoc_id(db);
        let summary = id
            .crate_body(db)
            .paths
            .get(&id.idx(db))
            .expect("Struct not found in a rustdoc crate");

        assert_eq!(summary.kind, ItemKind::Struct);
        summary.path.join("::")
    }

    /// Unqualified struct name, e.g. `Drop`.
    #[salsa::tracked(returns(ref))]
    pub fn name(self, db: &'db dyn Db) -> String {
        let id = self.rustdoc_id(db);
        let summary = id
            .crate_body(db)
            .paths
            .get(&id.idx(db))
            .expect("Struct not found in a rustdoc crate");

        assert_eq!(summary.kind, ItemKind::Struct);
        summary
            .path
            .last()
            .expect("Struct must have a name")
            .to_string()
    }

    /// Mangled fully qualified name, e.g. `std__mem__Drop`.
    #[salsa::tracked(returns(ref))]
    pub fn mangled_fqn(self, db: &'db dyn Db) -> String {
        let id = self.rustdoc_id(db);
        let summary = id
            .crate_body(db)
            .paths
            .get(&id.idx(db))
            .expect("Struct not found in a rustdoc crate");

        assert_eq!(summary.kind, ItemKind::Struct);
        mangle_path(&summary.path)
    }

    /// Binding mangled name, e.g. `__binding_struct__std__mem__Drop`.
    #[salsa::tracked(returns(ref))]
    pub fn binding_name(self, db: &'db dyn Db) -> String {
        format!("__binding_struct__{}", self.mangled_fqn(db))
    }

    /// Paired tethering struct name, e.g.
    /// `__tethered_struct__std__mem__Drop`.
    #[salsa::tracked(returns(ref))]
    pub fn tethered_name(self, db: &'db dyn Db) -> String {
        format!("__tethered_struct__{}", self.mangled_fqn(db))
    }

    /// Returns the rustdoc ID for this struct.
    pub fn rustdoc_id(self, db: &'db dyn Db) -> RustdocId<'db> {
        self.inner(db).rustdoc_id(db)
    }

    /// Returns the path to this struct.
    pub fn path(self, db: &'db dyn Db) -> &'db [String] {
        self.inner(db).path(db)
    }

    /// Returns the parent module ID of this item, or `None` if the item
    /// is a top-level module.
    pub fn parent_mod_id(self, db: &'db dyn Db) -> Option<IrId<'db>> {
        self.inner(db).parent_mod_id(db)
    }
}

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
    /// Returns the name of the binding struct, e.g.
    /// `__binding_struct__std_mem_Drop`
    pub fn binding_name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).binding_name(db)
    }

    /// Returns the name of the tethered struct, e.g.
    /// `__tethered_struct__std_mem_Drop`
    pub fn tethered_name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).tethered_name(db)
    }

    /// Returns the name of the struct, e.g. `Drop` for `std::mem::Drop`.
    pub fn name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).name(db)
    }

    /// Returns the fully qualified name of the struct, e.g.
    /// `std::mem::Drop`.
    pub fn fqn(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).fqn(db)
    }

    /// Returns the struct as a [`Type`].
    pub fn as_type(&self, db: &'db dyn Db) -> Type<'db> {
        Type::Struct(self.id(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<ShapeField<'db>> {
        match self.shape(db) {
            Shape::Unit => vec![],
            Shape::Tuple(fields) => fields.iter().flatten().cloned().collect(),
            Shape::Plain { fields, .. } => fields.clone(),
        }
    }
}
