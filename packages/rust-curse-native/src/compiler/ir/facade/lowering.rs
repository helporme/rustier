use crate::compiler::{
    db::Db,
    inputs::FilePaths,
    ir::{
        facade::{FacadeImport, FacadeImportItem, FacadeType},
        shim::ShimType,
    },
};

#[salsa::tracked]
impl<'db> FacadeImport<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_path(self, db: &'db dyn Db) -> String {
        let paths = FilePaths::get(db);

        let originating_from_path = self.originating_from(db).file_path(db);
        let to_path = match self.item(db) {
            FacadeImportItem::FfiFunction(_) | FacadeImportItem::FfiClass(_) => &paths.ffi_js_file,
            FacadeImportItem::FacadeClass(class) => class.module(db).file_path(db),
        };

        originating_from_path
            .relative_path(to_path)
            .expect("relative path must exist")
    }
}

impl<'db> ShimType<'db> {
    /// Lowers a shim type to facade.
    ///
    /// Do not make this function public, it just happened that we can lower the
    /// shim type to facade type directly, but it's not neccessary a thing in
    /// future.
    pub(super) fn lower_facade(self, db: &'db dyn Db) -> FacadeType<'db> {
        match self {
            ShimType::f64 => FacadeType::number,
            ShimType::String => FacadeType::string,
            ShimType::bool => FacadeType::boolean,
            ShimType::Class { inner, .. } => FacadeType::Class(inner.facade(db)),
        }
    }
}
