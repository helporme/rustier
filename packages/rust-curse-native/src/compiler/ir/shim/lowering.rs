use crate::compiler::{
    db::Db,
    ir::{rust::Impl, shim::ShimFunctionAssociated},
};

#[salsa::tracked]
impl<'db> Impl<'db> {
    /// Methods of this class.
    pub(super) fn lower_as_methods(
        self,
        db: &'db dyn Db,
    ) -> impl Iterator<Item = ShimFunctionAssociated<'db>> + 'db {
        self.functions(db)
            .iter()
            .filter(|function| function.receiver(db).is_some() || function.name(db) != "prototype")
            .map(move |function| ShimFunctionAssociated::new(db, *function))
    }
}
