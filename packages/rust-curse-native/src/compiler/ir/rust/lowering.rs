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
use ra_ap_hir::Adt;

use super::Span;
use crate::compiler::{
    FilePath,
    db::Db,
    diag::{DiagSource, DiagSpan},
};

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
) -> (DiagSource, Option<DiagSpan>) {
    (
        span.map(|span| span.diag_source(db)).into(),
        span.map(|span| span.diag_span(db)),
    )
}

pub(super) fn lower_item_span<T: ra_ap_hir::HasSource + Copy>(
    db: &dyn Db,
    item: T,
    crate_: ra_ap_hir::Crate,
) -> Option<Span> {
    let source = item.source_with_range(db)?;
    lower_span(
        db,
        crate_,
        source.file_id,
        source.value.0.start().into(),
        source.value.0.end().into(),
    )
}

fn lower_span(
    db: &dyn Db,
    crate_: ra_ap_hir::Crate,
    hir_file: ra_ap_hir::HirFileId,
    range_start: u32,
    range_end: u32,
) -> Option<Span> {
    let file_id = hir_file.original_file(db).file_id(db);
    let abs_path = file_path(db, file_id)?;
    let crate_root = file_path(db, crate_.root_file(db))?;
    let crate_dir = crate_root.as_path().parent()?;
    let src_path = abs_path
        .as_path()
        .strip_prefix(crate_dir)
        .unwrap_or_else(|_| abs_path.as_path())
        .to_owned();
    let begin = db.line_column(file_id, range_start.into()).ok()?;
    let end = db.line_column(file_id, range_end.into()).ok()?;

    Some(Span {
        abs_path,
        src_path,
        begin: (begin.0 as usize + 1, begin.1 as usize + 1),
        end: (end.0 as usize + 1, end.1 as usize + 1),
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
