//! Utilities for collecting diagnostics produced while lowering rustdoc items.

use rust_curse::{
    codegen::{
        db::Db,
        rustdoc::{lower_mod, lower_mod_fns, lower_mod_submods, Id},
    },
    diag::Diag,
};

/// Collects diagnostics from lowering every possible item in the given module.
pub fn collect_diags_mod<'db>(db: &'db dyn Db, id: Id<'db>) -> Vec<Diag> {
    let mut diags = vec![];

    // Lower mod itself.
    let _ = lower_mod(db, id);
    diags.extend(lower_mod::accumulated::<Diag>(db, id).into_iter().cloned());

    // Lower each fn in the module.
    let _ = lower_mod_fns(db, id);
    diags.extend(
        lower_mod_fns::accumulated::<Diag>(db, id)
            .into_iter()
            .cloned(),
    );

    // Lower each submodule in the module.
    diags.extend(
        lower_mod_submods(db, id)
            .iter()
            .flat_map(|mod_| collect_diags_mod(db, mod_.id(db).rustdoc_id(db))),
    );

    diags
}
