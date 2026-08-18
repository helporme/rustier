/// Concrete code generation database.
#[salsa::db]
#[derive(Clone, Default)]
pub struct DbImpl {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for DbImpl {}

/// Salsa database used by code generation queries.
#[salsa::db]
pub trait Db: salsa::Database {}

#[salsa::db]
impl Db for DbImpl {}
