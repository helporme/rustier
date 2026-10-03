mod callables;
mod crates;
mod enums;
mod fields;
mod functions;
mod generics;
mod impls;
mod modules;
mod shapes;
mod structs;
mod type_impls;
mod types;

use self::{
    enums::lower_enum,
    fields::{FieldLowerError, lower_field},
    functions::lower_function,
    impls::lower_impl,
    shapes::lower_shape,
    structs::lower_struct,
    types::{TypeLowerErrorUnsupported, instantiate_type, is_clonable, lower_type},
};
use miette::SourceSpan;
use ra_ap_hir::{Adt, GenericParam as HirGenericParam, HirDisplay as _};

use super::{Crate, Function, FunctionParent, GenArg, GenericParam, Rust, Span, TypeOrigin};
use crate::compiler::{db::Db, diag::DiagSource, path::FilePath};

impl<'db> TypeOrigin<'db> {
    pub(super) fn hir(self, db: &'db dyn Db) -> ra_ap_hir::Type<'db> {
        match self {
            Self::Field(field) => field.hir(db).ty(db),
            Self::QualifiedField(field) => instantiate_type(
                db,
                field.declaration(db).hir(db).ty(db),
                field.target(db).hir(db).type_arguments(),
            ),
            Self::FunctionParameter { function, index } => {
                function.hir(db).params_without_self(db)[index].ty().clone()
            }
            Self::FunctionReturn(function) => function.hir(db).ret_type(db),
            Self::CallableParameter { callable, index } => callable.instantiate(
                db,
                callable.declaration(db).hir(db).params_without_self(db)[index]
                    .ty()
                    .clone(),
            ),
            Self::CallableReturn(callable) => {
                callable.instantiate(db, callable.declaration(db).hir(db).ret_type(db))
            }
            Self::CallableReceiver(callable) => callable.instantiate(
                db,
                callable
                    .declaration(db)
                    .hir(db)
                    .self_param(db)
                    .expect("receiver")
                    .ty(db),
            ),
            Self::TypeArgument { type_, index } => type_
                .hir(db)
                .type_arguments()
                .nth(index)
                .expect("type argument"),
            Self::FunctionReceiver(function) => function
                .hir(db)
                .self_param(db)
                .expect("receiver origin requires a self parameter")
                .ty(db),
            Self::Struct(type_) => instantiate_type(
                db,
                type_.def(db).hir(db).ty(db),
                type_.gen_args(db).iter().filter_map(|arg| match arg {
                    GenArg::Type(type_) => Some(type_.hir(db)),
                    GenArg::Lifetime(_) => None,
                }),
            ),
            Self::Enum(type_) => instantiate_type(
                db,
                type_.def(db).hir(db).ty(db),
                type_.gen_args(db).iter().filter_map(|arg| match arg {
                    GenArg::Type(type_) => Some(type_.hir(db)),
                    GenArg::Lifetime(_) => None,
                }),
            ),
            Self::TypeParameter(param) => param.hir(db).ty(db),
            Self::Borrowed(reference) => reference.inner(db).hir(db).add_reference(
                db,
                if reference.is_mutable(db) {
                    ra_ap_hir::Mutability::Mut
                } else {
                    ra_ap_hir::Mutability::Shared
                },
            ),
            Self::Dereferenced(type_) => type_
                .hir(db)
                .as_reference_inner()
                .expect("dereferenced origin requires a reference"),
        }
    }
}

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
    // Diagnostics retain the declaration name without generic arguments.
    let declaration = Function::new(db, function);
    let parent = match declaration.parent(db) {
        FunctionParent::Module(module) => module.path(db).fqn(db).to_owned(),
        FunctionParent::Impl(impl_) => {
            let hir = impl_.hir(db);
            let self_ty = hir.self_ty(db);
            match self_ty.as_adt() {
                Some(adt) => adt_fqn(db, adt),
                None => self_ty
                    .display(db, hir.module(db).krate(db).to_display_target(db))
                    .to_string(),
            }
        }
    };
    format!("{parent}::{}", declaration.name(db))
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
            Self::Lifetime(param) => HirGenericParam::LifetimeParam(param.hir(db)),
        }
    }
}
