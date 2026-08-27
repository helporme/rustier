mod crates;
mod enums;
mod fields;
mod functions;
mod mods;
mod shapes;
mod structs;
mod types;

use self::{
    enums::lower_enum,
    fields::{FieldLowerError, lower_field},
    functions::lower_function,
    shapes::lower_shape,
    structs::lower_struct,
    types::{TypeLowerErrorUnsupported, is_clonable, lower_type},
};
use miette::SourceSpan;
use ra_ap_hir::Adt;

use super::Span;
use crate::compiler::{FilePath, db::Db, diag::DiagSource};

pub(super) fn function_fqn(db: &dyn Db, function: ra_ap_hir::Function) -> String {
    let module = function.module(db);
    let crate_ = module.krate(db);
    let name = function
        .name(db)
        .display(db, crate_.edition(db))
        .to_string();
    item_fqn(db, module, name)
}

pub(super) fn adt_fqn(db: &dyn Db, adt: Adt) -> String {
    let module = adt.module(db);
    let crate_ = module.krate(db);
    let name = adt.name(db).display(db, crate_.edition(db)).to_string();
    item_fqn(db, module, name)
}

fn item_fqn(db: &dyn Db, module: ra_ap_hir::Module, name: String) -> String {
    let crate_ = module.krate(db);
    let mut segments = Vec::new();
    if let Some(crate_name) = crate_.display_name(db) {
        segments.push(crate_name.to_string());
    }
    segments.extend(
        module
            .path_segments(db)
            .map(|segment| segment.display(db, crate_.edition(db)).to_string()),
    );
    segments.push(name);
    segments.join("::")
}

pub(super) fn diagnostic_location(
    db: &dyn Db,
    span: Option<&Span>,
) -> (DiagSource, Option<SourceSpan>) {
    (
        span.map(|span| span.src(db))
            .map(Into::into)
            .unwrap_or_default(),
        span.map(|span| (span.offset, span.length).into()),
    )
}

pub(super) fn lower_item_span<T: ra_ap_hir::HasSource>(db: &dyn Db, item: T) -> Option<Span> {
    let source = item.source_with_range(db)?;
    lower_span(
        db,
        source.file_id,
        source.value.0.start().into(),
        source.value.0.end().into(),
    )
}

fn lower_span(
    db: &dyn Db,
    hir_file: ra_ap_hir::HirFileId,
    range_start: u32,
    range_end: u32,
) -> Option<Span> {
    let file_id = hir_file.original_file(db).file_id(db);
    let src_path = file_path(db, file_id)?;

    Some(Span {
        src_path,
        offset: range_start as usize,
        length: range_end.saturating_sub(range_start) as usize,
    })
}

fn file_path(db: &dyn Db, file_id: ra_ap_vfs::FileId) -> Option<FilePath> {
    vfs_path(db, file_id).map(FilePath::from)
}

fn vfs_path(db: &dyn Db, file_id: ra_ap_vfs::FileId) -> Option<ra_ap_vfs::VfsPath> {
    let source_root_id = db.file_source_root(file_id).source_root_id(db);
    let source_root = db.source_root(source_root_id).source_root(db);
    source_root.path_for_file(&file_id).cloned()
}
