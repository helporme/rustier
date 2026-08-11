use crate::{
    compiler::{
        codegen::fragment::{FragmentOnce, Printer, doubles, fragment},
        db::Db,
        inputs::CompilationMetadata,
        ir::facade::{FacadeImportItem, FacadeMod},
    },
    frag,
};

#[salsa::tracked]
impl<'db> FacadeMod<'db> {
    pub(super) fn render_with(self, db: &'db dyn Db) -> impl FragmentOnce {
        FacadeModRenderer { mod_: self, db }
    }
}

#[derive(Copy, Clone)]
struct FacadeModRenderer<'db> {
    mod_: FacadeMod<'db>,
    db: &'db dyn Db,
}

impl<'db> FragmentOnce for FacadeModRenderer<'db> {
    fn apply(self, printer: &mut Printer) {
        todo!()
    }
}

impl<'db> FacadeModRenderer<'db> {
    fn render(self) -> impl FragmentOnce {}
}
