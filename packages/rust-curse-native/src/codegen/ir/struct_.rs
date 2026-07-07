use crate::codegen::{
    db::Db,
    ir::{mangle::mangle_path, Id, Type},
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

#[salsa::tracked(returns(ref))]
fn struct_mangled_name<'db>(db: &'db dyn Db, id: Id<'db>) -> String {
    let path = id.path(db);
    format!("__cursed_struct__{}", mangle_path(path))
}

#[salsa::tracked(returns(ref))]
fn struct_fqn<'db>(db: &'db dyn Db, id: Id<'db>) -> String {
    id.path(db).join("::")
}

impl<'db> Struct<'db> {
    /// Returns the mangled name of the struct e.g.
    /// `__cursed_struct__std_mem_Drop`
    pub fn mangled_name(&self, db: &'db dyn Db) -> &'db str {
        struct_mangled_name(db, self.id(db))
    }

    /// Returns the name of the struct, e.g. `Drop` for `std::mem::Drop`.
    pub fn name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db)
            .path(db)
            .last()
            .expect("struct must have a name")
    }

    /// Returns the fully qualified name of the struct, e.g.
    /// `std::mem::Drop`.
    pub fn fqn(&self, db: &'db dyn Db) -> &'db str {
        struct_fqn(db, self.id(db))
    }
}
