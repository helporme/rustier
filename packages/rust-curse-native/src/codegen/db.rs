#[salsa::db]
pub trait Db: salsa::Database {}

#[salsa::db]
#[derive(Clone, Default)]
pub struct DbImpl {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for DbImpl {}

#[salsa::db]
impl Db for DbImpl {}
