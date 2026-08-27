use crate::compiler::{
    codegen::fragment::{FragmentPrint, frag, spaced},
    db::Db,
    ir::facade::FacadeMod,
};

#[salsa::tracked]
impl<'db> FacadeMod<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db) -> String {
        let mut imports = self.imports(db).iter().copied().collect::<Vec<_>>();

        imports.sort_by(|left, right| {
            (left.path(db), left.item(db).symbol(db))
                .cmp(&(right.path(db), right.item(db).symbol(db)))
        });

        let imports = spaced(imports.into_iter().map(|import| import.render(db)));
        let declarations = spaced(
            self.classes(db)
                .iter()
                .map(|class| class.render(db))
                .chain(self.fns(db).iter().map(|function| function.render(db))),
        );

        frag!(
            {imports}@@
            {declarations}
        )
        .print()
    }
}
