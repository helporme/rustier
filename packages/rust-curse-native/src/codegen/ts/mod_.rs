//! Compiles all supported Rust module items into a TypeScript declaration file.

use std::collections::BTreeSet;

use crate::codegen::{
    db::Db,
    fragment::{FragmentOnce, FragmentPrint, fragment, spaced},
    js::imports::compile_import_stmt,
    rustdoc::{
        RustdocId,
        mod_::{lower_mod, lower_mod_fns, lower_mod_structs},
    },
    ts::{
        fn_::{compile_fn, compile_fn_required_imports},
        jsdoc::compile_jsdoc,
        struct_::{compile_struct, compile_struct_required_imports},
    },
};

/// Compiles all supported items declared by a Rust module into its TypeScript
/// declaration file.
#[salsa::tracked(returns(ref))]
pub fn compile_mod<'db>(db: &'db dyn Db, mod_id: RustdocId<'db>) -> String {
    let mod_ = lower_mod(db, mod_id);
    let fns = lower_mod_fns(db, mod_id);
    let structs = lower_mod_structs(db, mod_id);

    let mut imports = BTreeSet::new();
    for fn_ in fns.iter() {
        compile_fn_required_imports(db, *fn_, &mut imports);
    }
    for struct_ in structs.iter() {
        compile_struct_required_imports(db, *struct_, &mut imports);
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
            fns.iter()
                .map(|fn_| compile_fn(db, *fn_))
                .chain(structs.iter().map(|struct_| compile_struct(db, *struct_))),
        )
        .apply(p);
    })
    .print()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::test::{assert_snap, test_db};

    #[test]
    fn empty_mod() {
        assert_snap!({
            @lang: "ts";

            let db = test_db! {};

            compile_mod(&db, db.rustdoc_id("")).to_string()
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

                pub fn mixed(
                    number: i32,
                    text: String,
                    local: Local,
                    external: models::External,
                ) -> models::External {
                    external
                }
            };

            compile_mod(&db, db.rustdoc_id("")).to_string()
        });
    }
}
