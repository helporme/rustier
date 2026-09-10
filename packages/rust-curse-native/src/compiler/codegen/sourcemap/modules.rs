//! Assembles TypeScript declaration mappings into Source Map v3 documents.

use serde::Serialize;

use crate::compiler::{
    codegen::sourcemap::mappings::{Mapping, compile_mappings, find_name_line},
    db::Db,
    ir::decals::{DecalClass, DecalMember, DecalModule},
};

/// A Source Map v3 document for generated TypeScript declarations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, salsa::Update)]
struct SourceMap {
    version: u8,
    file: String,
    #[serde(rename = "sourceRoot")]
    source_root: String,
    sources: Vec<String>,
    names: Vec<String>,
    mappings: String,
}

#[salsa::tracked]
impl<'db> DecalModule<'db> {
    /// Renders a Source Map v3 document for this module's TypeScript
    /// declarations.
    #[salsa::tracked(returns(ref))]
    pub fn render_sourcemap(self, db: &'db dyn Db) -> String {
        let mappings = self.render_sourcemap_mappings(db);
        let compiled = compile_mappings(mappings);
        let file = self
            .file_path(db)
            .as_path()
            .file_name()
            .map(|file_name| format!("{}.d.ts", file_name.to_string_lossy()))
            .unwrap_or_default();

        serde_json::to_string(&SourceMap {
            version: 3,
            file,
            source_root: String::new(),
            sources: compiled.sources,
            names: compiled.names,
            mappings: compiled.encoded,
        })
        .expect("serializing a source map cannot fail")
    }

    /// Renders mappings for all declarations in their TypeScript output order.
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_sourcemap_mappings(self, db: &'db dyn Db) -> Vec<Mapping> {
        let generated = self.render_ts(db);
        let mut mappings = Vec::new();
        let mut search_line = 0;

        for function in self.members(db).filter_map(|member| match member {
            DecalMember::Function(function) => Some(function),
            _ => None,
        }) {
            let generated_line = find_name_line(
                generated,
                search_line,
                "export function ",
                function.name(db),
                "(",
            )
            .expect("rendered TypeScript function declaration should exist");
            search_line = generated_line + 1;
            mappings.extend(
                function
                    .render_sourcemap_mappings(db, generated_line)
                    .iter()
                    .cloned(),
            );
        }

        for class in self
            .members(db)
            .filter_map(|member| match member {
                DecalMember::Class(class) => Some(class),
                _ => None,
            })
            .filter(|class| matches!(class, DecalClass::Plain(_)))
        {
            let generated_line = find_name_line(
                generated,
                search_line,
                "export class ",
                class.name(db),
                " {",
            )
            .expect("rendered TypeScript class declaration should exist");
            search_line = generated_line + 1;
            mappings.extend(
                class
                    .render_sourcemap_mappings(db, self, generated_line)
                    .iter()
                    .cloned(),
            );
        }

        for class in self
            .members(db)
            .filter_map(|member| match member {
                DecalMember::Class(class) => Some(class),
                _ => None,
            })
            .filter(|class| matches!(class, DecalClass::Discriminated(_)))
        {
            let generated_line =
                find_name_line(generated, search_line, "export type ", class.name(db), " =")
                    .expect("rendered TypeScript union declaration should exist");
            search_line = generated_line + 1;
            mappings.extend(
                class
                    .render_sourcemap_mappings(db, self, generated_line)
                    .iter()
                    .cloned(),
            );
        }

        mappings
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        compiler::codegen::sourcemap::test::{decals_by_path, root_decals, snapshot_module},
        test_db,
    };

    macro_rules! test_module {
        ($name:ident, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                let db = test_db! { $($source)* };
                let module = root_decals(&db);
                let snapshot = snapshot_module(&db, module);

                insta::with_settings!({
                    snapshot_path => "_snapshots/modules",
                    prepend_module_to_snapshot => false,
                }, {
                    insta::assert_snapshot!(snapshot);
                });
            }
        };
    }

    test_module!(empty_module, {});

    test_module!(mixed_module, {
        @file: "models.rs";
        pub mod models {
            #[derive(Clone)]
            pub struct External;
        }

        /// Transforms values.
        pub fn transform(value: i32, external: models::External) -> i32 {
            value
        }

        /// Stores values.
        pub struct Holder {
            /// A writable number.
            pub number: i32,
            /// A Rust-backed value.
            pub external: models::External,
        }

        pub struct Tuple(pub i32, String, pub Holder);

        /// A shape represented by several layouts.
        pub enum Shape {
            /// An acronym-prefixed request.
            HTTPRequest {
                /// A named field.
                response_code: i32,
            },
            /// Coordinates represented by tuple fields.
            Point(
                /// A Rust-backed value that cannot be replaced.
                Holder,
                i32,
            ),
            /// A variant without fields.
            Unit,
        }
    });

    test_module!(renamed_discriminator_and_empty_function, {
        pub enum Example {
            Value { r#type: String },
        }

        pub enum Never {}

        pub fn empty() {}
    });

    test_module!(inherent_struct_methods_cross_file, {
        #[derive(Clone)]
        pub struct Counter;

        @file: "counter_methods.rs";
        mod counter_methods {
            impl super::Counter {
                /// Creates a counter.
                pub fn create(value: i32) -> Self {
                    let _ = value;
                    Self
                }

                pub fn value(&self) -> i32 {
                    0
                }

                pub fn add(&mut self, amount: i32) {
                    let _ = amount;
                }

                pub fn consume(self) -> bool {
                    true
                }

                pub fn constructor(&self) -> bool {
                    true
                }

                pub fn prototype() -> bool {
                    true
                }
            }
        }
    });

    test_module!(inherent_enum_methods, {
        #[derive(Clone)]
        pub enum Status {
            Pending,
            Complete,
        }

        impl Status {
            /// Creates a pending status.
            pub fn pending() -> Self {
                Self::Pending
            }

            pub fn is_complete(&self) -> bool {
                matches!(self, Self::Complete)
            }

            pub fn complete(&mut self) {
                *self = Self::Complete;
            }

            pub fn into_complete(self) -> bool {
                matches!(self, Self::Complete)
            }
        }
    });

    #[test]
    fn external_module() {
        let db = test_db! {
            @file: "nested/items.rs";
            pub mod items {
                pub fn first() {}

                pub struct Value {
                    pub number: i32,
                }
            }
        };
        let module = decals_by_path(&db, "items");
        let snapshot = snapshot_module(&db, module);

        insta::with_settings!({
            snapshot_path => "_snapshots/modules",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!(snapshot);
        });
    }
}
