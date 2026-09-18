mod crates;
mod enums;
mod fields;
mod functions;
mod generics;
mod impls;
mod modules;
mod shapes;
mod structs;
mod types;

use self::{
    enums::lower_enum,
    fields::{FieldLowerError, lower_field},
    functions::lower_function,
    impls::lower_impl,
    shapes::lower_shape,
    structs::lower_struct,
    types::{TypeLowerErrorUnsupported, is_clonable, lower_type},
};
use miette::SourceSpan;
use ra_ap_hir::{Adt, GenericParam as HirGenericParam};

use super::{AsItem as _, Crate, Function, GenericParam, Rust, Span};
use crate::compiler::{db::Db, diag::DiagSource, path::FilePath};


#[salsa::tracked]
impl<'db> Rust<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower(db: &'db dyn Db) -> Self {
        let mut crates = ra_ap_hir::Crate::all(db);
        crates.sort_by_key(|crate_| crate_.is_builtin(db));
        Self::new(
            db,
            crates
                .into_iter()
                .map(|crate_| Crate::new(db, crate_))
                .collect::<Vec<_>>(),
        )
    }
}

pub(super) fn function_fqn(db: &dyn Db, function: ra_ap_hir::Function) -> String {
    Function::new(db, function).fqn(db).to_owned()
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


impl<'db> GenericParam<'db> {
    fn hir(self, db: &'db dyn Db) -> HirGenericParam {
        match self {
            Self::Type(param) => HirGenericParam::TypeParam(param.hir(db)),
            Self::Const(param) => HirGenericParam::ConstParam(param.hir(db)),
            Self::Lifetime(param) => HirGenericParam::LifetimeParam(param.hir(db)),
        }
    }
}