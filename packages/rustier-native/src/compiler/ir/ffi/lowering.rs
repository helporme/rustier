use crate::compiler::{
    db::Db,
    ir::{
        ffi::{FfiFunction, FfiParam, FfiType},
        interface::InterType,
    },
};

#[salsa::tracked]
impl<'db> FfiFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_params(self, db: &'db dyn Db) -> Vec<FfiParam<'db>> {
        self.runtime(db)
            .params(db)
            .iter()
            .map(|param| FfiParam::new(db, *param))
            .collect()
    }

    pub(super) fn lower_ret_type(self, db: &'db dyn Db) -> Option<FfiType<'db>> {
        self.runtime(db)
            .ret_type(db)
            .map(|type_| type_.lower_ffi(db))
    }
}

#[salsa::tracked]
impl<'db> FfiParam<'db> {
    pub(super) fn lower_type(self, db: &'db dyn Db) -> FfiType<'db> {
        self.runtime(db).type_.lower_ffi(db)
    }
}

impl<'db> InterType<'db> {
    pub(super) fn lower_ffi(self, db: &'db dyn Db) -> FfiType<'db> {
        match self {
            InterType::f64 => FfiType::number,
            InterType::String => FfiType::string,
            InterType::bool => FfiType::boolean,
            InterType::Class { inner, .. } => FfiType::Class(inner.ffi(db)),
        }
    }
}
