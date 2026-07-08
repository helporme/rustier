use rustdoc_types::ItemKind;

use crate::codegen::{
    db::Db,
    ir::{mangle::mangle_path, IrId, Type},
    rustdoc::RustdocId,
};

#[salsa::tracked(debug)]
pub struct StructField<'db> {
    #[returns(ref)]
    pub name: String,
    #[tracked]
    pub type_: Type,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

#[salsa::tracked]
impl<'db> StructField<'db> {
    /// Returns the binding name of the getter function, e.g. `__getter__foo`.
    #[salsa::tracked(returns(ref))]
    pub fn getter_binding_name(self, db: &'db dyn Db) -> String {
        format!("__getter__{}", self.name(db))
    }

    /// Returns the binding name of the setter function, e.g. `__setter__foo`.
    #[salsa::tracked(returns(ref))]
    pub fn setter_binding_name(self, db: &'db dyn Db) -> String {
        format!("__setter__{}", self.name(db))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum StructShape<'db> {
    Unit,
    // private fields are `None`
    Tuple(Vec<Option<StructField<'db>>>),
    Plain {
        fields: Vec<StructField<'db>>,
        has_stripped_fields: bool,
    },
}

#[salsa::tracked(debug)]
pub struct Struct<'db> {
    pub id: IrId<'db>,
    #[tracked]
    #[returns(ref)]
    pub shape: StructShape<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

impl<'db> Struct<'db> {
    /// Returns the name of the binding struct, e.g.
    /// `__cursed_struct__std_mem_Drop`
    pub fn bindings_name(&self, db: &'db dyn Db) -> &'db str {
        struct_binding_name_from_rustdoc_id(db, self.id(db).rustdoc_id(db))
    }

    /// Returns the name of the struct, e.g. `Drop` for `std::mem::Drop`.
    pub fn name(&self, db: &'db dyn Db) -> &'db str {
        struct_name_from_rustdoc_id(db, self.id(db).rustdoc_id(db))
    }

    /// Returns the fully qualified name of the struct, e.g.
    /// `std::mem::Drop`.
    pub fn fqn(&self, db: &'db dyn Db) -> &'db str {
        struct_fqn_from_rustdoc_id(db, self.id(db).rustdoc_id(db))
    }
}

/// Returns the mangled name of the struct e.g. `std_mem_Drop` for a rustdoc id
/// pointing to the struct.
#[salsa::tracked(returns(ref))]
pub fn struct_mangled_name_from_rustdoc_id<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> String {
    let summary = id
        .crate_body(db)
        .paths
        .get(&id.idx(db))
        .expect("Struct not found in a rustdoc crate");

    debug_assert_eq!(summary.kind, ItemKind::Struct);
    mangle_path(&summary.path)
}

/// Returns the binding name of the struct e.g. `__cursed_struct__std_mem_Drop`
/// for a rustdoc id pointing to the struct.
#[salsa::tracked(returns(ref))]
pub fn struct_binding_name_from_rustdoc_id<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> String {
    let summary = id
        .crate_body(db)
        .paths
        .get(&id.idx(db))
        .expect("Struct not found in a rustdoc crate");

    debug_assert_eq!(summary.kind, ItemKind::Struct);
    format!("__cursed_struct__{}", mangle_path(&summary.path))
}

/// Returns the fully qualified name of the struct e.g. `std::mem::Drop`
/// for a rustdoc id pointing to the struct.
#[salsa::tracked(returns(ref))]
pub fn struct_fqn_from_rustdoc_id<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> String {
    let summary = id
        .crate_body(db)
        .paths
        .get(&id.idx(db))
        .expect("Struct not found in a rustdoc crate");

    debug_assert_eq!(summary.kind, ItemKind::Struct);
    summary.path.join("::")
}

/// Returns the name of the struct e.g. `std::mem::Drop`
/// for a rustdoc id pointing to the struct.
#[salsa::tracked(returns(ref))]
pub fn struct_name_from_rustdoc_id<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> String {
    let summary = id
        .crate_body(db)
        .paths
        .get(&id.idx(db))
        .expect("Struct not found in a rustdoc crate");

    debug_assert_eq!(summary.kind, ItemKind::Struct);
    summary
        .path
        .last()
        .expect("Struct must have a name")
        .to_string()
}
