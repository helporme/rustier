use crate::compiler::{
    codegen::fragment::{FragmentPrint, frag, spaced},
    db::Db,
    ir::facade::{FacadeMember, FacadeModule},
};

#[salsa::tracked]
impl<'db> FacadeModule<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db) -> String {
        let mut imports = self.imports(db).iter().copied().collect::<Vec<_>>();

        imports.sort_by(|left, right| {
            (left.path(db), left.item(db).symbol(db))
                .cmp(&(right.path(db), right.item(db).symbol(db)))
        });

        let imports = spaced(imports.into_iter().map(|import| import.render(db)));
        let declarations = spaced(self.members(db).iter().filter_map(|member| match member {
            FacadeMember::Class(class) => Some(class.render(db)),
            FacadeMember::Function(function) => Some(function.render(db)),
            FacadeMember::Module(_) => None,
        }));

        frag!(
            {imports}@@
            {declarations}
        )
        .print()
    }
}
