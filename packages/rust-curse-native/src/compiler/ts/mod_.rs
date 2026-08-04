//! Compiles all supported Rust module items into a TypeScript declaration file.

use std::collections::BTreeSet;

use crate::compiler::{
    db::Db,
    fragment::{FragmentOnce, FragmentPrint, fragment, spaced},
    ir::Mod,
    js::imports::compile_import_stmt,
    ts::{
        enum_::{compile_enum, compile_enum_required_imports},
        fn_::{compile_fn, compile_fn_required_imports},
        jsdoc::compile_jsdoc,
        struct_::{compile_struct, compile_struct_required_imports},
    },
};

/// Compiles all supported items declared by a Rust module into its TypeScript
/// declaration file.
pub fn compile_mod<'db>(db: &'db dyn Db, mod_: Mod<'db>) -> String {
    let fns = mod_.fns(db);
    let structs = mod_.structs(db);
    let enums = mod_.enums(db);

    let mut imports = BTreeSet::new();
    for fn_ in fns.clone() {
        compile_fn_required_imports(db, fn_, &mut imports);
    }
    for struct_ in structs.clone() {
        compile_struct_required_imports(db, struct_, &mut imports);
    }
    for enum_ in enums.clone() {
        compile_enum_required_imports(db, enum_, &mut imports);
    }
    let has_imports = !imports.is_empty();

    fragment(move |p| {
        spaced(
            imports
                .into_iter()
                .map(|import| compile_import_stmt(db, import)),
        )
        .apply(p);

        if has_imports {
            "\n\n".apply(p);
        }

        if let Some(docs) = mod_.docs(db).as_deref() {
            // todo: move to ambient.d.ts and put a comment on declaration of the module
            // so it's will be shown in IDE when hovering over the module name in import
            compile_jsdoc(Some(docs)).apply(p);
        }

        spaced(
            fns.map(|fn_| compile_fn(db, fn_))
                .chain(structs.map(|struct_| compile_struct(db, struct_)))
                .chain(enums.map(|enum_| compile_enum(db, enum_))),
        )
        .apply(p);
    })
    .print()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{
        lower::rustdoc_to_ir::mods::lower_mod,
        test::{assert_snap, test_db},
    };

    #[test]
    fn empty_mod() {
        assert_snap!({
            @lang: "ts";

            let db = test_db! {};
            let mod_ = lower_mod(&db, db.rustdoc_id(""));

            compile_mod(&db, mod_)
        });
    }

    #[test]
    fn mixed_mod() {
        assert_snap!({
            @lang: "ts";

            let db = test_db! {
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
            };
            let mod_ = lower_mod(&db, db.rustdoc_id(""));

            compile_mod(&db, mod_)
        });
    }
}
