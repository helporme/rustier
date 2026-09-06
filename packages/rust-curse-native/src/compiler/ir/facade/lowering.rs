use crate::compiler::{
    db::Db,
    ir::{facade::FacadeType, shim::ShimType},
};

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
