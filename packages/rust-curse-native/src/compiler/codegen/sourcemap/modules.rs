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
        let generated = self.render_ts_mod(db);
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

        for template in self.members(db).filter_map(|member| match member {
            DecalMember::ClassTemplate(template) => Some(template),
            _ => None,
        }) {
            let template_mappings = template.render_sourcemap_mappings(db, self, search_line);

            if let Some(line) = template_mappings
                .iter()
                .map(|mapping| mapping.generated_line)
                .max()
            {
                search_line = line + 1;
            }

            mappings.extend(template_mappings.iter().cloned());
        }

        mappings.sort_by_key(|mapping| (mapping.generated_line, mapping.generated_column));

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
                let snapshot = ra_ap_hir::attach_db(&db, || {
                    let module = root_decals(&db);
                    snapshot_module(&db, module)
                });

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

    #[test]
    fn struct_plain_mappings_preserve_source_locations() {
        let db = test_db! {
            pub struct Unit;
            pub struct Empty {}
            pub struct Value {
                /// Writable field documentation.
                pub number: i32,
                /// Readonly field documentation.
                pub unit: Unit,
                pub hidden: i32,
            }
            impl Value {
                pub fn hidden(&self) -> i32 { self.hidden }
            }
            pub struct Tuple(pub i32, Unit, pub Unit);
        };
        ra_ap_hir::attach_db(&db, || {
            let module = root_decals(&db);
            let generated = module.render_ts_mod(&db);
            let lines = generated.lines().collect::<Vec<_>>();
            let mappings = module.render_sourcemap_mappings(&db);
            let name_mapping = |line: usize| {
                mappings
                    .iter()
                    .filter(|mapping| mapping.generated_line == line)
                    .max_by_key(|mapping| mapping.generated_column)
                    .expect("declaration should have a name mapping")
            };
            let same_source = |left: &super::Mapping, right: &super::Mapping| {
                assert_eq!(
                    (
                        &left.source,
                        left.original_line,
                        left.original_column,
                        &left.name
                    ),
                    (
                        &right.source,
                        right.original_line,
                        right.original_column,
                        &right.name
                    ),
                );
            };
            for name in ["Unit", "Empty", "Value", "Tuple"] {
                let class_line = lines
                    .iter()
                    .position(|line| *line == format!("export class {name} {{"))
                    .unwrap();
                let namespace_line = lines
                    .iter()
                    .position(|line| *line == format!("export declare namespace {name} {{"))
                    .unwrap();
                let plain_line = namespace_line + 1;
                same_source(name_mapping(class_line), name_mapping(namespace_line));
                same_source(name_mapping(class_line), name_mapping(plain_line));
                assert_eq!(
                    name_mapping(namespace_line).generated_column,
                    "export declare namespace ".len()
                );
                assert_eq!(
                    name_mapping(plain_line).generated_column,
                    "\texport type ".len()
                );
                if matches!(name, "Unit" | "Empty") {
                    assert_eq!(lines[plain_line], "\texport type Plain = {};");
                }
                for field_line in plain_line + 1..lines.len() {
                    if !lines[field_line].starts_with("\t\t") {
                        break;
                    }
                    if let Some(field) = mappings
                        .iter()
                        .filter(|mapping| mapping.generated_line == field_line)
                        .max_by_key(|mapping| mapping.generated_column)
                    {
                        let accessor_name = if field.name.parse::<usize>().is_ok() {
                            format!("[{}]", field.name)
                        } else {
                            field.name.clone()
                        };
                        let getter_line = (class_line + 1..namespace_line)
                            .find(|&line| {
                                lines[line].starts_with(&format!("\tpublic get {accessor_name}()"))
                            })
                            .unwrap();
                        same_source(name_mapping(getter_line), field);
                        let setter_line = (class_line + 1..namespace_line).find(|&line| {
                            lines[line].starts_with(&format!("\tpublic set {accessor_name}("))
                        });
                        if lines[field_line].starts_with("\t\treadonly ") {
                            assert!(setter_line.is_none());
                        } else {
                            same_source(name_mapping(setter_line.unwrap()), field);
                        }
                        assert!(
                            lines[field_line][field.generated_column..].starts_with(&field.name)
                        );
                    }
                }
            }
            let mut plain_fields = mappings
                .iter()
                .filter(|mapping| {
                    lines[mapping.generated_line].starts_with("\t\t")
                        && mapping.generated_column > 0
                })
                .map(|mapping| mapping.name.as_str())
                .collect::<Vec<_>>();
            plain_fields.sort_unstable();
            assert_eq!(plain_fields, ["0", "2", "hidden", "number", "unit"]);
        });
    }

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
        #[derive(Clone)]
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
                /// A writable Rust-backed value.
                external: models::External,
            },
            /// Coordinates represented by tuple fields.
            Point(
                /// A writable Rust-backed value.
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

    test_module!(enum_object_method_names, {
        pub enum StaticNames {
            Value,
        }

        impl StaticNames {
            pub fn new() -> Self {
                Self::Value
            }

            pub fn constructor() -> bool {
                true
            }
        }

        pub enum InstanceNames {
            Value,
        }

        impl InstanceNames {
            pub fn new(&self) -> bool {
                true
            }

            pub fn constructor(&self) -> bool {
                true
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
        let snapshot = ra_ap_hir::attach_db(&db, || {
            let module = decals_by_path(&db, "items");
            snapshot_module(&db, module)
        });

        insta::with_settings!({
            snapshot_path => "_snapshots/modules",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!(snapshot);
        });
    }
}
