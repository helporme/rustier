use crate::compiler::{
    codegen::fragment::{FragmentPrint, frag, spaced},
    db::Db,
    ir::facade::{FacadeMember, FacadeModule},
};

#[salsa::tracked]
impl<'db> FacadeModule<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db) -> String {
        let mut imports = self.imports(db).to_vec();

        imports.sort_by(|left, right| {
            (left.from_path(db), left.item(db).symbol(db))
                .cmp(&(right.from_path(db), right.item(db).symbol(db)))
        });

        let imports = spaced(imports.into_iter().map(|import| import.render(db)));
        let declarations = spaced(self.members(db).filter_map(|member| match member {
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
