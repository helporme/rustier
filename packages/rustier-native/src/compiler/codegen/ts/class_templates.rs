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
            export type ~{name}<{params()}> =@{idented(instance_branches)};@@

            export ~{scope.declare_prefix()}namespace ~{name}~{{
                export type Class<{params()}> =@{idented(class_branches)};@@

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

#[cfg(test)]
mod tests {
    use crate::{
        assert_ts_snap,
        compiler::{codegen::fragment::FragmentOnce as _, ir::rust::Rust},
    };

    macro_rules! test_template {
        ($name:ident, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_ts_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let root = Rust::get(db)
                            .shim_complete(db)
                            .crates(db)
                            .next()
                            .expect("test crate should be lowered")
                            .root(db)
                            .decals(db);

                        root.render_ts_mod(db).apply(p);
                    },
                }
            }
        };
    }

    test_template!(primitive_keys, {
        pub struct Wrap<T>(pub T);

        pub fn consume(
            boolean: &Wrap<bool>,
            character: &Wrap<char>,
            i8_value: &Wrap<i8>,
            u8_value: &Wrap<u8>,
            i16_value: &Wrap<i16>,
            u16_value: &Wrap<u16>,
            i32_value: &Wrap<i32>,
            u32_value: &Wrap<u32>,
            f32_value: &Wrap<f32>,
            f64_value: &Wrap<f64>,
            string: &Wrap<String>,
        ) {
        }
    });

    test_template!(class_and_nested_keys, {
        #[derive(Clone)]
        pub struct Holder(pub u32);

        #[derive(Clone)]
        pub struct Wrap<T>(pub T);

        pub fn consume(number: &Wrap<u32>, holder: &Wrap<Holder>, nested: &Wrap<Wrap<u32>>) {}
    });

    test_template!(cross_module_keys, {
        pub mod models {
            #[derive(Clone)]
            pub struct Holder(pub u32);

            #[derive(Clone)]
            pub struct Boxed<T>(pub T);

            #[derive(Clone)]
            pub enum Status {
                Ready,
            }
        }

        #[derive(Clone)]
        pub struct Wrap<T>(pub T);

        pub fn consume(boxed: &Wrap<models::Boxed<models::Holder>>, status: &Wrap<models::Status>) {
        }
    });

    test_template!(multiple_generic_arguments, {
        #[derive(Clone)]
        pub struct Pair<T, U>(pub T, pub U);

        pub fn consume(first: &Pair<u32, bool>, second: &Pair<bool, u32>) {}
    });

    test_template!(generic_constructor, {
        #[doc = "A generic wrapper."]
        #[derive(Clone)]
        pub struct Wrap<T>(pub T);

        impl<T> Wrap<T> {
            #[doc = "Constructs a wrapper."]
            pub fn new(value: T) -> Self {
                Self(value)
            }

            pub fn identity(value: T) -> T {
                value
            }
        }

        impl Wrap<u32> {
            pub fn number_only(&self) -> bool {
                true
            }
        }

        pub fn consume(number: &Wrap<u32>, string: &Wrap<String>, nested: &Wrap<Wrap<u32>>) {}
    });

    test_template!(generic_enum, {
        pub struct Holder(pub u32);

        #[doc = "An event carrying a generic value."]
        #[derive(Clone)]
        pub enum Event<T> {
            #[doc = "Carries a value."]
            Value(T),
            Empty,
        }

        impl<T> Event<T> {
            pub fn new(value: T) -> Self {
                Self::Value(value)
            }

            pub fn is_empty(&self) -> bool {
                false
            }
        }

        pub fn consume(number: &Event<u32>, holder: &Event<Holder>) {}
    });

    test_template!(coerce_availability, {
        pub struct Holder(pub u32);

        #[derive(Clone)]
        pub struct Wrap<T>(pub T);

        pub fn consume(number: &Wrap<u32>, holder: &Wrap<Holder>) {}
    });

    test_template!(borrowed_values_excluded, {
        pub struct Wrap<T>(pub T);
        pub enum Event<T> {
            Value(T),
        }

        pub fn consume(value: &Wrap<&u32>, event: &Event<&u32>) {}
    });

    test_template!(unreachable_template, {
        pub struct Wrap<T>(T);
    });

    #[test]
    fn ambient_templates() {
        assert_ts_snap! {
            source: {
                pub mod models {
                    #[derive(Clone)]
                    pub struct Holder(pub u32);

                    #[derive(Clone)]
                    pub struct Boxed<T>(pub T);

                    #[derive(Clone)]
                    pub enum Status {
                        Ready,
                    }
                }

                #[derive(Clone)]
                pub struct Wrap<T>(pub T);

                #[derive(Clone)]
                pub enum Event<T> {
                    Value { value: T },
                    Empty,
                }

                pub fn consume(
                    wrap: &Wrap<models::Boxed<models::Holder>>,
                    status: &Wrap<models::Status>,
                    event: &Event<String>,
                ) {}
            },
            expression: |db, p| {
                Rust::get(db)
                    .shim_complete(db)
                    .decals(db)
                    .render_ambient_ts_mods(db)
                    .apply(p);
            },
        }
    }
}
