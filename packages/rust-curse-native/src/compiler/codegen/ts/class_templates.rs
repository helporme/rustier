use crate::compiler::{
    codegen::{
        fragment::{FragmentOnce, FragmentPrint, frag, fragment, idented, spaced},
        ts::{DeclarationScope, jsdoc::render_jsdoc, types::render_inline_list},
    },
    db::Db,
    ir::decals::{DecalClassTemplate, DecalGeneric},
};

#[salsa::tracked]
impl<'db> DecalClassTemplate<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_in_scope_ts(self, db: &'db dyn Db, scope: DeclarationScope) -> String {
        let name = self.name(db);
        let params = || render_inline_list(self.params(db).iter());
        let constrained_params = || {
            render_inline_list(self.params(db).iter().enumerate().map(|(index, param)| {
            frag!({param}~extends~{self.render_param_constraint_ts(db, index)})
        }))
        };
        let classes = spaced(
            self.mappings(db)
                .iter()
                .map(|(_, class)| class.render_in_scope_ts(db, scope)),
        );
        let class_branches = self.render_branches_ts(db, TemplateProjection::Class);
        let instance_branches = self.render_branches_ts(db, TemplateProjection::Instance);
        let plain_branches = self.render_branches_ts(db, TemplateProjection::Plain);
        let coerce_branches = self.render_branches_ts(db, TemplateProjection::Coerce);
        let args = fragment(move |p| {
            for (index, (generics, _)) in self.mappings(db).iter().enumerate() {
                if index > 0 {
                    frag!(~|~).apply(p);
                }

                frag!([{
                    render_inline_list(generics.iter().map(|generic| generic.render_ts(db)))
                }])
                .apply(p);
            }

            if self.mappings(db).is_empty() {
                frag!(never).apply(p);
            }
        });
        let return_args = render_inline_list(
            self.params(db)
                .iter()
                .enumerate()
                .map(|(index, _)| frag!(Args[{ index }])),
        );

        frag!(
            {classes}@@

            {render_jsdoc(self.docs(db).map(String::as_str))}
            export type ~{name}<{params()}> = ~_{name}<{params()}> & Partial<_{name}<{params()}>>;@
            type ~_{name}<{params()}> =@{idented(instance_branches)};@@

            export ~{scope.declare_prefix()}namespace ~{name}~{{
                export type Class<{params()}> = _Class<{params()}> & Partial<_Class<{params()}>>;@
                type _Class<{params()}> =@{idented(class_branches)};@@

                export type Plain<{constrained_params()}> =@{idented(plain_branches)};@@

                export type Coerce<{constrained_params()}> =@{idented(coerce_branches)};
            }}@@

            {render_jsdoc(self.docs(db).map(String::as_str))}
            export function ~{name}<Args extends ~{args}>(...args: Args):~{name}.Class<{return_args}>;
        ).print()
    }

    fn render_param_constraint_ts(self, db: &'db dyn Db, index: usize) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            let mut seen = std::collections::HashSet::new();

            for (generics, _) in self.mappings(db) {
                let generic = &generics[index];

                if !seen.insert(generic) {
                    continue;
                }

                if seen.len() > 1 {
                    frag!(~|~).apply(p);
                }

                generic.render_ts(db).apply(p);
            }

            if seen.is_empty() {
                frag!(never).apply(p);
            }
        })
    }

    fn render_branches_ts(
        self,
        db: &'db dyn Db,
        projection: TemplateProjection,
    ) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            if self.params(db).len() > 1 {
                for param in self.params(db) {
                    frag!({param}~extends unknown ?@).apply(p);
                }
            }

            for (generics, class) in self.mappings(db) {
                self.render_condition_ts(db, generics).apply(p);

                match projection {
                    TemplateProjection::Instance => frag!(~{class.name(db)}).apply(p),
                    TemplateProjection::Class => frag!(~typeof ~{class.name(db)}).apply(p),
                    TemplateProjection::Plain => {
                        frag!(@{idented(class.render_plain_body_ts(db))}).apply(p)
                    }
                    TemplateProjection::Coerce => {
                        frag!(@{idented(class.render_coerce_body_ts(db))}).apply(p)
                    }
                }

                frag!(~:@).apply(p);
            }

            frag!(never).apply(p);

            if self.params(db).len() > 1 {
                for _ in self.params(db) {
                    frag!(~: never).apply(p);
                }
            }
        })
    }

    pub(in crate::compiler::codegen) fn render_condition_ts(
        self,
        db: &'db dyn Db,
        generics: &'db [DecalGeneric<'db>],
    ) -> impl FragmentOnce + 'db {
        let params = render_inline_list(self.params(db).iter());
        let generics = render_inline_list(generics.iter().map(|generic| generic.render_ts(db)));

        fragment(move |p| {
            if self.params(db).len() == 1 {
                frag!({params}~extends ~{generics}~?).apply(p);
            } else {
                frag!([{params}]~extends~[{generics}]~?).apply(p);
            }
        })
    }
}

#[derive(Clone, Copy)]
enum TemplateProjection {
    Instance,
    Class,
    Plain,
    Coerce,
}
