//! Assembles TypeScript declaration mappings into Source Map v3 documents.

use std::fmt;

use serde::Serialize;

use crate::codegen::{
    db::Db,
    ir::{ItemIdExt, Mod},
    ts::{
        mod_::compile_mod as compile_ts_mod,
        sourcemap::{
            Mapping,
            enum_::compile_enum_mappings,
            fn_::compile_fn_mappings,
            mappings::{compile_mappings, find_name_line},
            struct_::compile_struct_mappings,
        },
    },
};

/// A Source Map v3 document for generated TypeScript declarations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, salsa::Update)]
pub struct SourceMap {
    version: u8,
    file: String,
    #[serde(rename = "sourceRoot")]
    source_root: String,
    sources: Vec<String>,
    names: Vec<String>,
    mappings: String,
}

impl fmt::Display for SourceMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        f.write_str(&json)
    }
}

/// Compiles an IR module into a TypeScript declaration source map.
pub fn compile_mod<'db>(db: &'db dyn Db, mod_: Mod<'db>) -> SourceMap {
    let file = mod_
        .span(db)
        .as_ref()
        .and_then(|span| span.src_path.file_name())
        .map(|file_name| format!("{}.d.ts", file_name.to_string_lossy()))
        .unwrap_or_default();
    let mappings = compile_mod_mappings(db, mod_);
    let compiled_mappings = compile_mappings(&mappings);

    SourceMap {
        version: 3,
        file,
        source_root: String::new(),
        sources: compiled_mappings.sources,
        names: compiled_mappings.names,
        mappings: compiled_mappings.encoded,
    }
}

/// Compiles all emitted items in a module into mappings at their generated
/// locations.
pub fn compile_mod_mappings<'db>(db: &'db dyn Db, mod_: Mod<'db>) -> Vec<Mapping> {
    let generated = compile_ts_mod(db, mod_);
    let mut mappings = Vec::new();
    let mut search_line = 0;

    for fn_ in mod_.fns(db) {
        let generated_line = find_name_line(
            &generated,
            search_line,
            "export function ",
            fn_.name(db),
            "(",
        )
        .expect("compiled TypeScript function declaration should exist");
        search_line = generated_line + 1;
        mappings.extend(compile_fn_mappings(db, fn_, generated_line).iter().cloned());
    }

    for struct_ in mod_.structs(db) {
        let generated_line = find_name_line(
            &generated,
            search_line,
            "export class ",
            struct_.name(db),
            " {",
        )
        .expect("compiled TypeScript class declaration should exist");
        search_line = generated_line + 1;
        mappings.extend(
            compile_struct_mappings(db, struct_, generated_line)
                .iter()
                .cloned(),
        );
    }

    for enum_ in mod_.enums(db) {
        let generated_line = find_name_line(
            &generated,
            search_line,
            "export type ",
            enum_.name(db),
            " =",
        )
        .expect("compiled TypeScript enum type declaration should exist");
        search_line = generated_line + 1;
        mappings.extend(
            compile_enum_mappings(db, enum_, generated_line)
                .iter()
                .cloned(),
        );
    }

    mappings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        db::DbImpl,
        lower::rustdoc_to_ir::mods::lower_mod,
        test::{assert_snap, test_db},
        ts::sourcemap::test::{normalize_mappings, normalize_source, numbered_lines},
    };

    fn compile_test_mod(db: DbImpl, path: &str) -> String {
        let mod_id = db.rustdoc_id(path);
        let crate_dir = &mod_id.crate_(&db).dir;
        let mod_ = lower_mod(&db, mod_id);
        let generated_source = compile_ts_mod(&db, mod_);
        let generated = numbered_lines(&generated_source);
        let compiled_mappings = compile_mod_mappings(&db, mod_);
        let mappings = normalize_mappings(crate_dir, &compiled_mappings);
        let mut source_map = compile_mod(&db, mod_);
        source_map.sources = source_map
            .sources
            .iter()
            .map(|source| normalize_source(crate_dir, source))
            .collect();
        let source_map = serde_json::to_string_pretty(&source_map).unwrap();

        format!("TypeScript:\n{generated}\n\nSource map:\n{source_map}\n\nMappings:\n{mappings:#?}")
    }

    #[test]
    fn empty_mod() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {};

            compile_test_mod(db, "")
        });
    }

    #[test]
    fn documented_mod() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                //! First module documentation line.
                //! Second module documentation line.

                /// Function documentation.
                pub fn foo() {}
            };

            compile_test_mod(db, "")
        });
    }

    #[test]
    fn multiple_fn_mod() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                pub fn many_params(
                    first: i32,
                    second: i32,
                    third: i32,
                    fourth: i32,
                    fifth: i32,
                ) {}

                pub fn after() {}
            };

            compile_test_mod(db, "")
        });
    }

    #[test]
    fn importing_mod() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                @file: "models.rs";
                pub mod models {
                    #[derive(Clone)]
                    pub struct External;
                }

                pub fn consume(value: models::External) {}

                pub struct Holder {
                    pub external: models::External,
                }
            };

            compile_test_mod(db, "")
        });
    }

    #[test]
    fn mixed_fn_struct_mod() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                pub struct First {
                    pub value: i32,
                }

                pub fn transform(value: i32) -> i32 {
                    value
                }

                /// A unit value.
                pub struct Last;
            };

            compile_test_mod(db, "")
        });
    }

    #[test]
    fn enum_after_other_items() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                //! Module documentation before generated imports and declarations.

                @file: "models.rs";
                pub mod models {
                    #[derive(Clone)]
                    pub struct External;
                }

                pub fn before() {}

                pub struct Value {
                    pub number: i32,
                }

                /// The first choice emitted after functions and structs.
                pub enum FirstChoice {
                    Imported { external: models::External },
                }

                /// The second choice preserves enum declaration order.
                pub enum SecondChoice {
                    Empty {},
                }
            };

            compile_test_mod(db, "")
        });
    }

    #[test]
    fn structs_only_mod() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                pub struct First {
                    pub value: i32,
                }

                pub struct Second;
            };

            compile_test_mod(db, "")
        });
    }

    #[test]
    fn external_mod() {
        assert_snap!({
            @lang: "text";

            let db = test_db! {
                @file: "nested/items.rs";
                pub mod items {
                    pub fn first() {}

                    pub struct Value {
                        pub number: i32,
                    }
                }
            };

            compile_test_mod(db, "items")
        });
    }
}
