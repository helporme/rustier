use crate::compiler::{
    codegen::{
        fragment::{FragmentOnce, FragmentPrint, frag, fragment, singles, spaced},
        ts::{DeclarationScope, jsdoc::render_jsdoc},
    },
    db::Db,
    ir::decals::{DecalClass, DecalMember, DecalModule},
};

#[salsa::tracked]
impl<'db> DecalModule<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render_ts_mod(self, db: &'db dyn Db) -> String {
        self.render_ts_in(db, DeclarationScope::Module)
    }

    pub(super) fn render_ts_in(self, db: &'db dyn Db, scope: DeclarationScope) -> String {
        let members = self.members(db).collect::<Vec<_>>();
        let functions = members
            .iter()
            .filter_map(|member| match member {
                DecalMember::Function(function) => Some(*function),
                _ => None,
            })
            .collect::<Vec<_>>();
        let classes = members
            .iter()
            .filter_map(|member| match member {
                DecalMember::Class(class) => Some(*class),
                _ => None,
            })
            .collect::<Vec<_>>();
        let templates = members
            .iter()
            .filter_map(|member| match member {
                DecalMember::ClassTemplate(template) => Some(*template),
                _ => None,
            })
            .collect::<Vec<_>>();

        if functions.is_empty() && classes.is_empty() && templates.is_empty() {
            // make sure that returned stirng is always valid typescript module
            return frag!(export {};).print();
        }

        let imports = self.imports(db);
        let has_imports = !imports.is_empty();
        let functions = functions.into_iter().map(|function| function.render_ts(db));

        let plain_classes = classes
            .iter()
            .copied()
            .filter(|class| matches!(class, DecalClass::Plain(_)))
            .map(|class| class.render_in_scope_ts(db, scope));

        let discriminated_classes = classes
            .iter()
            .copied()
            .filter(|class| matches!(class, DecalClass::Discriminated(_)))
            .map(|class| class.render_in_scope_ts(db, scope));

        fragment(move |p| {
            spaced(imports.iter().map(|import| {
                frag!(
                    import type ~{{@
                        {import.class(db).type_name(db).name(db)}@
                    }}~ from ~{singles(import.path(db))};
                )
            }))
            .apply(p);

            if has_imports {
                frag!(@@).apply(p);
            }

            if let Some(docs) = self.docs(db) {
                render_jsdoc(Some(docs)).apply(p);
            }

            spaced(
                functions
                    .chain(plain_classes)
                    .chain(discriminated_classes)
                    .chain(
                        templates
                            .into_iter()
                            .map(|template| template.render_in_scope_ts(db, scope)),
                    ),
            )
            .apply(p);

            if scope == DeclarationScope::Module {
                frag!(@@export {};).apply(p);
            }
        })
        .print()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_ts_snap,
        compiler::{codegen::fragment::FragmentOnce as _, ir::rust::Rust},
    };

    macro_rules! test_module {
        ($name:ident, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_ts_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let db = &*db;
                        let root = Rust::get(db)
                            .interface_complete(db)
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

    test_module!(mixed_module, {
        @file: "models.rs";
        pub mod models {
            #[derive(Clone)]
            pub struct External;
        }

        #[derive(Clone)]
        pub struct Local;

        pub struct Mixed {
            pub number: i32,
            pub text: String,
            pub local: Local,
            pub external: models::External,
        }

        pub enum Choice {
            Empty,
            Value { number: i32 },
        }

        pub fn mixed(
            number: i32,
            text: String,
            local: Local,
            external: models::External,
        ) -> models::External {
            external
        }
    });

    test_module!(documented_enum, {
        @file: "models.rs";
        pub mod models {
            #[derive(Clone)]
            pub struct External;
        }

        pub struct Local;

        /// A shape represented by one of several layouts.
        pub enum Shape {
            /// A circle with named fields.
            Circle {
                /// The circle radius.
                radius: f64,
                /// A local Rust-backed value.
                local: Local,
                /// An imported Rust-backed value.
                external: models::External,
            },
            /// Coordinates represented by tuple fields.
            Point(i32, i32),
            /// A variant without fields.
            Unit,
            /// Checks acronym conversion.
            HTTPRequest,
        }
    });

    test_module!(renamed_discriminator_and_empty_function, {
        pub enum Example {
            Value { r#type: String },
        }

        pub fn empty() {}
    });

    test_module!(distinct_enum_brands, {
        pub enum Wave {
            Sine(f32),
        }

        pub enum OtherWave {
            Sine(f32),
        }

        // Ordinary Rust names must not collide with synthetic brand symbols.
        pub struct WaveBrand;
    });

    test_module!(coerce_imports_field_type, {
        @file: "models.rs";
        pub mod models {
            #[derive(Clone)]
            pub struct Point {
                pub x: u32,
            }
        }

        #[derive(Clone)]
        pub struct Circle {
            pub center: models::Point,
            pub radius: f64,
        }

        impl Circle {
            // The field and its coerce input retain the imported type.
            pub fn center(&self) -> bool {
                true
            }
        }
    });

    test_module!(coerce_unavailable, {
        pub struct NoClone {
            pub value: u32,
        }

        #[derive(Clone)]
        pub struct Private(pub u32, u32);

        #[derive(Clone)]
        #[non_exhaustive]
        pub struct NonExhaustive {
            pub value: u32,
        }

        #[derive(Clone)]
        pub enum Blocked {
            #[non_exhaustive]
            Unit,
        }
    });
}
