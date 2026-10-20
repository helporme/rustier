use crate::compiler::{
    codegen::{
        fragment::{FragmentPrint, frag, spaced},
        js::JsCodegenPaths,
    },
    db::Db,
    ir::facade::{FacadeMember, FacadeModule, FacadeModuleEntry},
};

impl<'db> FacadeModuleEntry<'db> {
    pub fn render_js(self, db: &'db dyn Db, paths: JsCodegenPaths) -> String {
        match self {
            Self::Source(module) => module.render(db, paths).to_owned(),
            Self::Runtime(module) => module.render_js().to_owned(),
        }
    }
}

#[salsa::tracked]
impl<'db> FacadeModule<'db> {
    /// Renders the module as a string.
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db, paths: JsCodegenPaths) -> String {
        let mut imports = self.imports(db).to_vec();

        imports.sort_by_cached_key(|import| {
            (
                import.render_path(db, paths.clone()),
                import.item(db).name(db),
            )
        });

        let imports = spaced(
            imports
                .into_iter()
                .map(|import| import.render(db, paths.clone())),
        );

        let declarations = spaced(self.members(db).iter().copied().filter_map(
            |member| match member {
                FacadeMember::Class(class) => Some(class.render(db)),
                FacadeMember::ClassTemplate(template) => Some(template.render(db)),
                FacadeMember::Function(function) => Some(function.render(db)),
                FacadeMember::Trait(trait_) => Some(trait_.render_js(db)),
                FacadeMember::Module(_) => None,
            },
        ));

        frag!(
            {imports}@@
            {declarations}
        )
        .print()
    }
}
