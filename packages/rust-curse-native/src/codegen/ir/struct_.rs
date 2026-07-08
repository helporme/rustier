use rustdoc_types::ItemKind;

use crate::codegen::{
    db::Db,
    ir::{mangle::mangle_path, Id, Type},
    rustdoc,
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

#[salsa::tracked(returns(ref))]
fn struct_field_getter_name<'db>(db: &'db dyn Db, field: StructField<'db>) -> String {
    format!("__getter__{}", field.name(db))
}

#[salsa::tracked(returns(ref))]
fn struct_field_setter_name<'db>(db: &'db dyn Db, field: StructField<'db>) -> String {
    format!("__setter__{}", field.name(db))
}

impl<'db> StructField<'db> {
    pub fn getter_name(&self, db: &'db dyn Db) -> &'db String {
        struct_field_getter_name(db, *self)
    }

    pub fn setter_name(&self, db: &'db dyn Db) -> &'db String {
        struct_field_setter_name(db, *self)
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

/// A struct.
#[salsa::tracked(debug)]
pub struct Struct<'db> {
    pub id: Id<'db>,
    #[tracked]
    #[returns(ref)]
    pub shape: StructShape<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

impl<'db> Struct<'db> {
    /// Returns the cursed name of the struct e.g.
    /// `__cursed_struct__std_mem_Drop`
    pub fn cursed_name(&self, db: &'db dyn Db) -> &'db str {
        struct_cursed_name_from_rustdoc_id(db, self.id(db).rustdoc_id(db))
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
pub fn struct_mangled_name_from_rustdoc_id<'db>(db: &'db dyn Db, id: rustdoc::Id<'db>) -> String {
    let summary = id
        .crate_body(db)
        .paths
        .get(&id.idx(db))
        .expect("Struct not found in a rustdoc crate");

    debug_assert_eq!(summary.kind, ItemKind::Struct);
    mangle_path(&summary.path)
}

/// Returns the cursed name of the struct e.g. `__cursed_struct__std_mem_Drop`
/// for a rustdoc id pointing to the struct.
#[salsa::tracked(returns(ref))]
pub fn struct_cursed_name_from_rustdoc_id<'db>(db: &'db dyn Db, id: rustdoc::Id<'db>) -> String {
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
pub fn struct_fqn_from_rustdoc_id<'db>(db: &'db dyn Db, id: rustdoc::Id<'db>) -> String {
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
pub fn struct_name_from_rustdoc_id<'db>(db: &'db dyn Db, id: rustdoc::Id<'db>) -> String {
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
