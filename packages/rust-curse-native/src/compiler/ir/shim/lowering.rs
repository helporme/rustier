use crate::compiler::{
    db::Db,
    ir::{rust::Type, shim::ShimFunctionAssociated},
};

#[salsa::tracked]
impl<'db> Type<'db> {
    /// Methods of this class.
    pub(super) fn lower_as_methods(
        self,
        db: &'db dyn Db,
    ) -> impl Iterator<Item = ShimFunctionAssociated<'db>> + 'db {
        self.impls(db)
            .iter()
            .flat_map(|impl_| impl_.functions(db))
            .filter(|function| {
                let declaration = function.declaration(db);
                declaration.receiver(db).is_some() || declaration.name(db) != "prototype"
            })
            .map(move |function| {
                ShimFunctionAssociated::new(
                    db,
                    function
                        .as_callible(db)
                        .expect("generic functions cannot reach the shim"),
                )
            })
    }
}
