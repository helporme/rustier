use crate::codegen::db::Db;

use super::*;

// #[salsa::tracked(returns(ref), cycle_fn = recover, cycle_initial = initial)]
pub fn lower_rustdoc<'db>(db: &'db dyn Db, rcrate: Rustdoc) {}
