use crate::compiler::{
    db::Db,
    ir::{facade::FacadeType, interface::InterType},
};

impl<'db> InterType<'db> {
    /// Lowers an interface type to facade.
    ///
    /// Do not make this function public, it just happened that we can lower the
    /// interface type to facade type directly, but it's not neccessary a thing
    /// in future.
    pub(super) fn lower_facade(self, db: &'db dyn Db) -> FacadeType<'db> {
        match self {
            InterType::f64 => FacadeType::number,
            InterType::String => FacadeType::string,
            InterType::bool => FacadeType::boolean,
            InterType::Class { inner, .. } => FacadeType::Class(inner.facade(db)),
        }
    }
}
