use crate::compiler::{
    db::Db,
    ir::{
        ffi::{FfiFunction, FfiParam, FfiType},
        shim::ShimType,
    },
};

#[salsa::tracked]
impl<'db> FfiFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_params(self, db: &'db dyn Db) -> Vec<FfiParam<'db>> {
        self.shim(db)
            .params(db)
            .iter()
            .map(|param| param.ffi(db))
            .collect()
    }

    pub(super) fn lower_ret_type(self, db: &'db dyn Db) -> Option<FfiType<'db>> {
        self.shim(db).ret_type(db).map(|type_| type_.lower_ffi(db))
    }
}

#[salsa::tracked]
impl<'db> FfiParam<'db> {
    pub(super) fn lower_type(self, db: &'db dyn Db) -> FfiType<'db> {
        self.shim(db).type_(db).lower_ffi(db)
    }
}

impl<'db> ShimType<'db> {
    pub(super) fn lower_ffi(self, db: &'db dyn Db) -> FfiType<'db> {
        match self {
            ShimType::f64 => FfiType::number,
            ShimType::String => FfiType::string,
            ShimType::bool => FfiType::boolean,
            ShimType::Class { inner, .. } => FfiType::Class(inner.ffi(db)),
        }
    }
}
