use crate::{
    compiler::{
        codegen::fragment::{FragmentPrint, frag, spaced},
        db::Db,
        ir::facade::FacadeMod,
    },
};

#[salsa::tracked]
impl<'db> FacadeMod<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db) -> String {
        let imports = spaced(self.imports(db).iter().map(|im| im.render(db)));
        let fns = spaced(self.fns(db).iter().map(|f| f.render(db)));

        frag!(
            {imports}@@
            {fns}
        )
        .print()
    }
}
