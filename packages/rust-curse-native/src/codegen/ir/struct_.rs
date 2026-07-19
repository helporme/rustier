use rustdoc_types::ItemKind;

use crate::codegen::{
    db::Db,
    ir::{
        IrId,
        mangle::mangle_path,
        shape::{Shape, ShapeField},
        type_::Type,
    },
};

#[salsa::tracked(debug)]
pub struct Struct<'db> {
    pub id: IrId<'db>,
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
    pub fn getter_binding_name_on_struct(self, db: &'db dyn Db, struct_id: IrId<'db>) -> String {
        let _ = struct_id;
        format!("__getter__{}", self.name(db))
    }

    /// Returns the struct field binding setter function name, e.g.
    /// `__setter__foo`.
    #[salsa::tracked(returns(ref))]
    pub fn setter_binding_name_on_struct(self, db: &'db dyn Db, struct_id: IrId<'db>) -> String {
        let _ = struct_id;
        format!("__setter__{}", self.name(db))
    }
}

#[salsa::tracked]
impl<'db> Struct<'db> {
    /// Returns the name of the binding struct, e.g.
    /// `__cursed_struct__std_mem_Drop`
    pub fn binding_name(&self, db: &'db dyn Db) -> &'db str {
        struct_binding_name_from_ir_id(db, self.id(db))
    }

    /// Returns the name of the tethered struct, e.g.
    /// `__tethered_struct__std_mem_Drop`
    pub fn tethered_name(&self, db: &'db dyn Db) -> &'db str {
        struct_tethered_name_from_ir_id(db, self.id(db))
    }

    /// Returns the name of the struct, e.g. `Drop` for `std::mem::Drop`.
    pub fn name(&self, db: &'db dyn Db) -> &'db str {
        struct_name_from_ir_id(db, self.id(db))
    }

    /// Returns the fully qualified name of the struct, e.g.
    /// `std::mem::Drop`.
    pub fn fqn(&self, db: &'db dyn Db) -> &'db str {
        struct_fqn_from_ir_id(db, self.id(db))
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

/// Returns the mangled name of the struct e.g. `std_mem_Drop` for a rustdoc id
/// pointing to the struct.
#[salsa::tracked(returns(ref))]
pub fn struct_mangled_name_from_ir_id<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    let id = id.rustdoc_id(db);
    let summary = id
        .crate_body(db)
        .paths
        .get(&id.idx(db))
        .expect("Struct not found in a rustdoc crate");

    assert_eq!(summary.kind, ItemKind::Struct);
    mangle_path(&summary.path)
}

/// Returns the binding name of the struct e.g. `__cursed_struct__std_mem_Drop`
/// for a rustdoc id pointing to the struct.
#[salsa::tracked(returns(ref))]
pub fn struct_binding_name_from_ir_id<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    let id = id.rustdoc_id(db);
    let summary = id
        .crate_body(db)
        .paths
        .get(&id.idx(db))
        .expect("Struct not found in a rustdoc crate");

    assert_eq!(summary.kind, ItemKind::Struct);
    format!("__cursed_struct__{}", mangle_path(&summary.path))
}

/// Returns the handle name of the struct e.g. `__tethered_struct__std_mem_Drop`
/// for a rustdoc id pointing to the struct.
#[salsa::tracked(returns(ref))]
pub fn struct_tethered_name_from_ir_id<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    let id = id.rustdoc_id(db);
    let summary = id
        .crate_body(db)
        .paths
        .get(&id.idx(db))
        .expect("Struct not found in a rustdoc crate");

    assert_eq!(summary.kind, ItemKind::Struct);
    format!("__tethered_struct__{}", mangle_path(&summary.path))
}

/// Returns the fully qualified name of the struct e.g. `std::mem::Drop`
/// for a rustdoc id pointing to the struct.
#[salsa::tracked(returns(ref))]
pub fn struct_fqn_from_ir_id<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    let id = id.rustdoc_id(db);
    let summary = id
        .crate_body(db)
        .paths
        .get(&id.idx(db))
        .expect("Struct not found in a rustdoc crate");

    assert_eq!(summary.kind, ItemKind::Struct);
    summary.path.join("::")
}

/// Returns the name of the struct e.g. `std::mem::Drop`
/// for a rustdoc id pointing to the struct.
#[salsa::tracked(returns(ref))]
pub fn struct_name_from_ir_id<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    let id = id.rustdoc_id(db);
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
