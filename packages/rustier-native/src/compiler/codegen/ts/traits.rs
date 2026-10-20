use crate::compiler::{
    codegen::{
        fragment::{FragmentOnce, FragmentPrint, doubles, frag, fragment, idented},
        ts::{DeclarationScope, jsdoc::render_jsdoc, types::render_inline_list},
    },
    db::Db,
    ir::facade::{
        FacadeClass, FacadeGeneric, FacadeModule,
        imports_ts::{FacadeImportItemTs, FacadeImportTs},
        traits::{FacadeTrait, FacadeTraitArgument},
    },
};

#[salsa::tracked]
impl<'db> FacadeTrait<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_ts(self, db: &'db dyn Db, scope: DeclarationScope) -> String {
        let name = self.name(db);
        let decl_name = self.decl_name(db);
        let id = self.src(db).path(db).fqn(db);
        let arity = self.src(db).generic_params(db).len();
        let module = self
            .src(db)
            .module(db)
            .facade_reachable_from(db, self.usages(db));
        let declaration = fragment(move |p| {
            if arity == 0 {
                frag!(
                    {scope.declare_prefix()}const ~{decl_name}:~{{
                        readonly $trait:~{doubles(id)};@
                        readonly $args: readonly ~[];
                    }};
                )
                .apply(p);
            } else {
                let constraint = fragment(move |p| {
                    for (index, mapping) in self.specializations(db).iter().enumerate() {
                        if index > 0 {
                            frag!(~|~).apply(p);
                        }

                        frag!(readonly ~[{
                            render_inline_list(mapping.arguments.iter().map(|argument| argument.render_key_ts(db, module)))
                        }]).apply(p);
                    }
                });
                let immutable_argument = idented(frag!(
                    ? Arg extends readonly ~unknown[]~? readonly ~[...Arg]~: Arg@
                    : never
                ));

                frag!(
                    {scope.declare_prefix()}function ~{decl_name}<const Args extends ~{constraint}>(...types: Args):~{{
                        readonly $trait:~{doubles(id)};@
                        readonly $args:~{{
                            readonly ~[Index in keyof Args]: Args[Index]~extends infer Arg@
                            {immutable_argument};
                        }};
                    }};
                ).apply(p);
            }
        });
        frag!(
            {render_jsdoc(self.src(db).docs(db).map(String::as_str))}
            {declaration}@@export~{{{decl_name}~as~{name}}};
        )
        .print()
    }
}

impl<'db> FacadeTraitArgument<'db> {
    fn render_key_ts(
        &'db self,
        db: &'db dyn Db,
        module: FacadeModule<'db>,
    ) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            Self::Value(FacadeGeneric::Symbol(symbol)) => doubles(symbol).apply(p),
            Self::Value(FacadeGeneric::Class(class)) => {
                render_class_key(db, module, *class).apply(p)
            }
            Self::Reference { prefix, class } => {
                frag!(readonly ~[{doubles(prefix)},~{render_class_key(db, module, *class)}])
                    .apply(p);
            }
        })
    }
}

fn render_class_key<'db>(
    db: &'db dyn Db,
    module: FacadeModule<'db>,
    class: FacadeClass<'db>,
) -> impl FragmentOnce + 'db {
    let import = FacadeImportTs::new(db, module, FacadeImportItemTs::FacadeClass(class));
    let name = class.type_name(db);

    fragment(move |p| {
        if !class.is_template_instance(db) {
            frag!(typeof~).apply(p);
        }
        frag!(import({doubles(import.path(db))}).{name.name(db)}).apply(p);

        if let Some(arguments) = name.args(db) {
            let keys = render_inline_list(arguments.iter().map(move |argument| {
                fragment(move |p| match argument {
                    FacadeGeneric::Symbol(symbol) => doubles(symbol).apply(p),
                    FacadeGeneric::Class(class) => render_class_key(db, module, *class).apply(p),
                })
            }));

            frag!(.Class<{keys}>).apply(p);
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_ts_snap,
        compiler::{
            codegen::{
                fragment::{FragmentOnce as _, doubles, frag},
                ts::DeclarationScope,
            },
            ir::rust::Rust,
        },
    };

    macro_rules! test_trait {
        ($name:ident, $trait_name:literal, $scope:ident, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_ts_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let facade = Rust::get(db).facade_complete(db);
                        let trait_ = facade.traits(db).iter().copied()
                            .find(|trait_| trait_.name(db) == $trait_name)
                            .expect("trait descriptor should be reachable");
                        let scope = DeclarationScope::$scope;

                        if scope == DeclarationScope::AmbientModule {
                            let module = trait_.module(db).virtual_path(db);

                            frag!(
                                declare module ~{doubles(module)}~{{
                                    {trait_.render_ts(db, scope)}
                                }}
                            ).apply(p);
                        } else {
                            trait_.render_ts(db, scope).apply(p);
                        }
                    },
                }
            }
        };
    }

    test_trait!(clone_constant, "Clone", Module, {
        #[derive(Clone)]
        pub struct Widget;
    });

    test_trait!(primitive_specializations, "Convert", Module, {
        pub trait Convert<T> {
            fn convert(&self, value: T);
        }

        pub struct Widget;

        impl Convert<u32> for Widget {
            fn convert(&self, _value: u32) {}
        }

        impl Convert<bool> for Widget {
            fn convert(&self, _value: bool) {}
        }
    });

    test_trait!(class_key, "Probe", Module, {
        pub trait Probe<T> {
            fn probe(&self) -> bool;
        }

        pub struct Seed;
        pub struct Widget;

        impl Probe<Seed> for Widget {
            fn probe(&self) -> bool {
                true
            }
        }
    });

    test_trait!(reference_key, "Probe", Module, {
        pub trait Probe<T> {
            fn probe(&self) -> bool;
        }

        pub struct Seed;
        pub struct Widget;

        impl Probe<&mut Seed> for Widget {
            fn probe(&self) -> bool {
                true
            }
        }
    });

    test_trait!(borrowed_builtin_key, "Probe", Module, {
        pub trait Probe<T: ?Sized> {
            fn probe(&self) -> bool;
        }

        pub struct Widget;

        impl Probe<&str> for Widget {
            fn probe(&self) -> bool {
                true
            }
        }
    });

    test_trait!(nested_generic_class_key, "Probe", Module, {
        pub trait Probe<T> {
            fn probe(&self) -> bool;
        }

        pub struct Seed;
        pub struct Wrap<T>(T);
        pub struct Widget;

        impl Probe<Wrap<Wrap<Seed>>> for Widget {
            fn probe(&self) -> bool {
                true
            }
        }
    });

    test_trait!(correlated_arguments, "Pair", Module, {
        pub trait Pair<T, U> {
            fn pair(&self) -> bool;
        }

        pub struct Widget;

        impl Pair<u32, bool> for Widget {
            fn pair(&self) -> bool {
                true
            }
        }

        impl Pair<bool, u32> for Widget {
            fn pair(&self) -> bool {
                false
            }
        }
    });

    test_trait!(reserved_name, "default", Module, {
        pub trait r#default {
            fn check(&self) -> bool;
        }

        pub struct Widget;

        impl r#default for Widget {
            fn check(&self) -> bool {
                true
            }
        }
    });

    test_trait!(ambient_constant, "Clone", AmbientModule, {
        #[derive(Clone)]
        pub struct Widget;
    });
}
