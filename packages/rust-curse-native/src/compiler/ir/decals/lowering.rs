use crate::compiler::{
    db::Db,
    ir::{decals::DecalType, shim::ShimType},
};

impl<'db> ShimType<'db> {
    pub(super) fn lower_decals(self, db: &'db dyn Db) -> DecalType<'db> {
        match self {
            ShimType::f64 => DecalType::number,
            ShimType::String => DecalType::string,
            ShimType::bool => DecalType::boolean,
            ShimType::Class { inner, .. } => DecalType::Class {
                inner: inner.decals(db),
                coerced: false,
            },
        }
    }
}
