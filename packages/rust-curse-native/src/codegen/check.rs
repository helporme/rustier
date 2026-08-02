//! Module for running checks on IR for spotting common (but not critical)
//! mistakes and pitfalls when using rust-curse.

use miette::Diagnostic;
use thiserror::Error;

use crate::codegen::{db::Db, ir::mod_::Mod};

mod enum_js_name_collisions;
mod f32_percission_los;
pub use enum_js_name_collisions::EnumJsNameCollision;
pub use f32_percission_los::F32PrecisionLoss;

/// Runs all checks and returns their diagnostics.
///
/// The check is independent from [`crate::codegen::api::Api`], it runs
/// only on generated IR.
pub fn check<'db>(db: &'db dyn Db, modules: &[Mod<'db>]) -> Vec<Check> {
    f32_percission_los::check_f32_percission_los(db, modules)
        .into_iter()
        .map(Check::from)
        .chain(
            enum_js_name_collisions::check_enum_js_name_collisions(db, modules)
                .into_iter()
                .map(Check::from),
        )
        .collect()
}

/// A diagnostic emitted by an IR check.
#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum Check {
    #[error(transparent)]
    #[diagnostic(transparent)]
    F32PrecisionLoss(#[from] F32PrecisionLoss),

    #[error(transparent)]
    #[diagnostic(transparent)]
    EnumJsNameCollision(#[from] EnumJsNameCollision),
}
