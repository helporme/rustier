//! Utilities for collecting diagnostics produced while lowering rustdoc items.

use miette::Diagnostic;
use rust_curse::codegen::{
    check,
    db::Db,
    diag::Diag,
    rustdoc::{
        mod_::{lower_mod, lower_mod_fns, lower_mod_structs, lower_mod_submods},
        RustdocId,
    },
};

/// Collects diagnostics from lowering every possible item in the given module
/// and from all IR checks.
pub fn collect_diags<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Vec<Diag> {
    let mut diags = collect_lower_diags_mod(db, id);

    diags.extend(check::check(db).iter().cloned().map(Diag::from));

    diags
}

fn collect_lower_diags_mod<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Vec<Diag> {
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

    // Lower each struct in the module.
    let _ = lower_mod_structs(db, id);
    diags.extend(
        lower_mod_structs::accumulated::<Diag>(db, id)
            .into_iter()
            .cloned(),
    );

    // Lower each submodule in the module.
    diags.extend(
        lower_mod_submods(db, id)
            .iter()
            .flat_map(|mod_| collect_lower_diags_mod(db, mod_.id(db).rustdoc_id(db))),
    );

    diags
}

pub fn assert_single_diag_without_related(diags: &[Diag], expected_code: &str) {
    assert_eq!(diags.len(), 1);
    assert_eq!(diag_code(diags[0].inner.as_ref()), expected_code);
    assert!(related(diags[0].inner.as_ref()).is_empty());
}

fn diag_code(diag: &dyn Diagnostic) -> String {
    diag.code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "<missing code>".to_string())
}

fn related(diag: &dyn Diagnostic) -> Vec<&dyn Diagnostic> {
    diag.related()
        .map(|related| related.collect())
        .unwrap_or_default()
}
