use crate::compiler::{
    codegen::fragment::{FragmentOnce, Printer},
    db::Db,
    ir::facade::FacadeFunction,
};

#[salsa::tracked]
impl<'db> FacadeFunction<'db> {
    pub(super) fn render_with(self, db: &'db dyn Db) -> impl FragmentOnce {
        FacadeFunctionRenderer { function: self, db }
    }
}

#[derive(Copy, Clone)]
struct FacadeFunctionRenderer<'db> {
    function: FacadeFunction<'db>,
    db: &'db dyn Db,
}

impl<'db> FragmentOnce for FacadeFunctionRenderer<'db> {
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db> FacadeFunctionRenderer<'db> {
    fn render(self) -> impl FragmentOnce {
        
    }
}
