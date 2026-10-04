use miette::{Diagnostic, SourceSpan};
use ra_ap_hir::{HasAttrs, Module};
use thiserror::Error;

use super::diagnostic_location;
use crate::compiler::{
    db::Db,
    diag::DiagSource,
    ir::rust::{Module as RustModule, Span},
};

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("unstable items are not supported")]
#[diagnostic(
    code(rust_curse::unsupported_unstable_item),
    help("use a stable API or remove #[unstable] from the source item"),
    severity(Warning)
)]
pub(super) struct UnstableItemError {
    #[source_code]
    source_code: DiagSource,
    #[label("this item is unstable")]
    span: Option<SourceSpan>,
}

pub(super) fn check_stability(
    db: &dyn Db,
    hir: impl HasAttrs + Copy,
    module: Module,
    item_span: impl FnOnce() -> Option<Span>,
) -> Result<(), UnstableItemError> {
    let span = if hir.attrs(db).is_unstable() {
        item_span()
    } else if let Some(module) = std::iter::successors(Some(module), |module| module.parent(db))
        .find(|module| module.attrs(db).is_unstable())
    {
        RustModule::new(db, module).span(db).cloned()
    } else {
        return Ok(());
    };
    let (source_code, span) = diagnostic_location(db, span.as_ref());

    Err(UnstableItemError { source_code, span })
}
