pub use ra_ap_ide_db::RootDatabase;

#[salsa::db]
pub trait Db: ra_ap_hir::db::HirDatabase {}

#[salsa::db]
impl Db for RootDatabase {}
