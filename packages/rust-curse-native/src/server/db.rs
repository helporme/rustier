#[salsa::db]
pub trait Db: salsa::Database {}

#[salsa::db]
#[derive(Clone, Default)]
pub struct ServerDb {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for ServerDb {}

#[salsa::db]
impl Db for ServerDb {}
