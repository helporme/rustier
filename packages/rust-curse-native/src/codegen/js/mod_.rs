//! Compiles all supported Rust module items into a facade JavaScript file.

use std::collections::BTreeSet;

use crate::codegen::{
    api::Api,
    db::Db,
    fragment::{FragmentPrint, frag, spaced},
    ir::{item::ItemId, mod_::ModId},
    js::{
        JsCodegenPaths,
        enum_::{compile_facade_class_for_enum, compile_facade_class_for_enum_required_imports},
        fn_::{compile_facade_fn, compile_facade_fn_required_imports},
        imports::compile_import_stmt,
        struct_::{
            compile_facade_class_for_struct, compile_facade_class_for_struct_required_imports,
        },
    },
    rustdoc::mod_::{lower_mod_enums, lower_mod_fns, lower_mod_structs},
};

/// Compiles all supported items declared by a Rust module into its facade
/// JavaScript file.
pub fn compile_js_file<'db>(db: &'db dyn Db, mod_id: ModId<'db>) -> &'db String {
    compile_js_file_tracked(db, mod_id.into())
}

#[salsa::tracked(returns(ref))]
fn compile_js_file_tracked<'db>(db: &'db dyn Db, mod_id: ItemId<'db>) -> String {
    let js_codegen_paths = JsCodegenPaths::get(db);

    let fns = lower_mod_fns(db, mod_id.rustdoc_id(db));
    let structs = lower_mod_structs(db, mod_id.rustdoc_id(db));
    let enums = lower_mod_enums(db, mod_id.rustdoc_id(db));

    let mut imports = BTreeSet::new();
    for fn_ in fns.iter() {
        compile_facade_fn_required_imports(db, *fn_, &mut imports);
    }
    for struct_ in structs.iter() {
        compile_facade_class_for_struct_required_imports(db, *struct_, &mut imports);
    }
    for enum_ in enums.iter() {
        compile_facade_class_for_enum_required_imports(db, *enum_, &mut imports);
    }
    let imports = spaced(
        imports
            .into_iter()
            .map(|import| compile_import_stmt(db, import)),
    );

    let fns = spaced(fns.iter().map(|fn_| compile_facade_fn(db, *fn_)));
    let structs = spaced(
        structs
            .iter()
            .map(|struct_| compile_facade_class_for_struct(db, *struct_)),
    );
    let enums = spaced(
        enums
            .iter()
            .map(|enum_| compile_facade_class_for_enum(db, *enum_)),
    );

    let init = Api::get(db).js_mod_init(db, js_codegen_paths.generated_bindings_path(db));
    frag!(
        {init}@
        {imports}@@
        {fns}@@
        {structs}@@
        {enums}
    )
    .print()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        api::{Api, ApiKind},
        db::DbImpl,
        js::JsCodegenPaths,
        test::{assert_snap, test_db},
    };

    fn compile_test_mod(mut db: DbImpl) -> String {
        Api::set(&mut db, ApiKind::Node);
        JsCodegenPaths::set(&mut db, "path/to/bindings".to_owned());

        let mod_id =
            ModId::new(&db, db.item_id("")).expect("the test database root item must be a module");
        compile_js_file(&db, mod_id).to_string()
    }

    #[test]
    fn empty_mod() {
        assert_snap!({
            @lang: "js";

            let db = test_db! {};

            compile_test_mod(db)
        });
    }

    #[test]
    fn mixed_mod() {
        assert_snap!({
            @lang: "js";

            let db = test_db! {
                @file: "models.rs";
                pub mod models {
                    #[derive(Clone)]
                    pub struct External;

                    #[derive(Clone)]
                    pub enum ExternalKind {
                        One,
                    }
                }

                #[derive(Clone)]
                pub struct Local;

                pub enum LocalEnum {
                    Unit,
                    Named {
                        local: Local,
                        external: models::External,
                    },
                    Tuple(models::ExternalKind),
                }

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
                    models::External
                }
            };

            compile_test_mod(db)
        });
    }
}
