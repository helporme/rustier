use crate::compiler::{
    db::Db,
    ir::ffi::{FfiFunction, FfiParam, FfiType},
};

#[salsa::tracked]
impl<'db> FfiFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_params(self, db: &'db dyn Db) -> Vec<FfiParam<'db>> {
        self.shim(db)
            .params(db)
            .iter()
            .copied()
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
