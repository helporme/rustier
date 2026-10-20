use crate::compiler::{
    codegen::fragment::{FragmentOnce, FragmentPrint, doubles, frag, fragment, spaced},
    db::Db,
    ir::facade::{
        FacadeGeneric,
        traits::{FacadeTrait, FacadeTraitArgument},
    },
};

#[salsa::tracked]
impl<'db> FacadeTrait<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_js(self, db: &'db dyn Db) -> String {
        let name = self.name(db);
        let decl_name = self.decl_name(db);
        let id = self.src(db).path(db).fqn(db);
        let arity = self.src(db).generic_params(db).len();
        let declaration = fragment(move |p| {
            if arity == 0 {
                frag!(
                    const ~{decl_name}~=~globalThis.Object.freeze({{
                        $trait:~{doubles(id)},@
                        $args: globalThis.Object.freeze([])
                    }});
                )
                .apply(p);
            } else {
                let branches = spaced(self.specializations(db).iter().map(move |mapping| {
                    let conditions = fragment(move |p| {
                        for (index, argument) in mapping.arguments.iter().enumerate() {
                            match argument {
                                FacadeTraitArgument::Reference { prefix, class } => {
                                    frag!(
                                        ~&&~globalThis.Array.isArray(types[{index}])
                                        ~&&~types[{index}].length === 2
                                        ~&&~types[{index}][0]~===~{doubles(prefix)}
                                        ~&&~types[{index}][1]~===~{class.import_alias(db)}
                                    )
                                    .apply(p);
                                }
                                FacadeTraitArgument::Value(_) => {
                                    frag!(~&&~types[{index}]~===~{argument.render_key_js(db)})
                                        .apply(p)
                                }
                            }
                        }
                    });
                    let arguments = fragment(move |p| {
                        for (index, argument) in mapping.arguments.iter().enumerate() {
                            if index > 0 {
                                frag!(,~).apply(p);
                            }

                            argument.render_key_js(db).apply(p);
                        }
                    });

                    frag!(
                        if~(types.length~===~{arity}{conditions})~{{
                            return globalThis.Object.freeze({{
                                $trait:~{doubles(id)},@
                                $args: globalThis.Object.freeze([{arguments}])
                            }});
                        }}
                    )
                }));

                frag!(
                    function ~{decl_name}(...types)~{{
                        {branches}@@
                        throw new globalThis.TypeError({doubles(("Unsupported generic arguments for ", name))});
                    }}
                ).apply(p);
            }
        });

        frag!({declaration}@@export~{{{decl_name}~as~{name}}};).print()
    }
}

impl<'db> FacadeTraitArgument<'db> {
    fn render_key_js(&'db self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            Self::Value(FacadeGeneric::Symbol(symbol)) => doubles(symbol).apply(p),
            Self::Value(FacadeGeneric::Class(class)) => class.import_alias(db).apply(p),
            Self::Reference { prefix, class } => {
                frag!(globalThis.Object.freeze([{doubles(prefix)},~{class.import_alias(db)}]))
                    .apply(p);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_js_snap,
        compiler::{
            codegen::{fragment::FragmentOnce as _, js::JsCodegenPaths},
            ir::rust::Rust,
        },
    };

    macro_rules! test_trait {
        ($name:ident, $trait_name:literal, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_js_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let facade = Rust::get(db).facade_complete(db);
                        let trait_ = facade.traits(db).iter().copied()
                            .find(|trait_| trait_.name(db) == $trait_name)
                            .expect("trait descriptor should be reachable");

                        trait_.render_js(db).apply(p);
                    },
                }
            }
        };
    }

    test_trait!(clone_constant, "Clone", {
        #[derive(Clone)]
        pub struct Widget;
    });

    test_trait!(primitive_specializations, "Convert", {
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

    test_trait!(class_key, "Probe", {
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

    test_trait!(reference_key, "Probe", {
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

    test_trait!(borrowed_builtin_key, "Probe", {
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

    test_trait!(nested_generic_class_key, "Probe", {
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

    test_trait!(correlated_arguments, "Pair", {
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

    test_trait!(reserved_name, "default", {
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

    #[test]
    fn trait_only_module_imports() {
        assert_js_snap! {
            source: {
                pub mod contracts {
                    pub trait Probe<T> {
                        fn probe(&self) -> bool;
                    }
                }

                pub struct Seed;
                pub struct Widget;

                impl contracts::Probe<&Seed> for Widget {
                    fn probe(&self) -> bool {
                        true
                    }
                }
            },
            expression: |db, p| {
                let root = Rust::get(db).facade_complete(db)
                    .crates(db).next().expect("test crate").root(db);
                let contracts = root.modules(db).next().expect("trait-only module");
                let paths = JsCodegenPaths {
                    abi_js_mod_id: "$rust/unstable!/node".to_owned(),
                };

                contracts.render(db, paths).apply(p);
            },
        }
    }
}
