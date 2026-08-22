use crate::{
    compiler::{
        codegen::{
            fragment::{FragmentOnce, FragmentPrint, fragment, singles, spaced},
            ts::jsdoc::render_jsdoc,
        },
        db::Db,
        ir::decals::{DecalClassKind, DecalMod},
    },
    frag,
};

#[salsa::tracked]
impl<'db> DecalMod<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render_ts(self, db: &'db dyn Db) -> String {
        let imports = self.ts_imports(db);
        let has_imports = !imports.is_empty();
        let functions = self.fns(db).iter().map(|function| function.render_ts(db));

        let plain_classes = self
            .classes(db)
            .iter()
            .filter(|&class| matches!(class.kind(db), DecalClassKind::Plain { .. }))
            .map(|class| class.render_ts(db));

        let discriminated_classes = self
            .classes(db)
            .iter()
            .filter(|&class| matches!(class.kind(db), DecalClassKind::DiscriminatedUnion { .. }))
            .map(|class| class.render_ts(db));

        fragment(move |p| {
            spaced(imports.into_iter().map(|(name, path)| {
                frag!(
                    import type ~{{@
                        {name}@
                    }}~ from ~{singles(path)};
                )
            }))
            .apply(p);

            if has_imports {
                frag!(@@).apply(p);
            }

            if let Some(docs) = self.docs(db) {
                render_jsdoc(Some(docs)).apply(p);
            }

            spaced(functions.chain(plain_classes).chain(discriminated_classes)).apply(p);
        })
        .print()
    }

    fn ts_imports(self, db: &'db dyn Db) -> std::collections::BTreeSet<(String, String)> {
        self.imports(db)
            .iter()
            .map(|import| {
                (
                    import.class(db).name(db).to_owned(),
                    import.path(db).to_owned(),
                )
            })
            .collect()
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
                            .crates(db)
                            .first()
                            .expect("test crate should be lowered")
                            .root(db)
                            .shim(db)
                            .decals(db);

                        root.render_ts(db).apply(p);
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
}
