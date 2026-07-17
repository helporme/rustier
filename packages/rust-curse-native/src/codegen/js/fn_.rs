use std::collections::BTreeSet;

use crate::{
    codegen::{
        db::Db,
        fragment::{FragmentOnce, comma_list, fragment},
        ir::fn_::Fn,
        js::{
            boundary::{
                compile_js_binding_value_to_facade_expr,
                compile_js_binding_value_to_facade_expr_required_imports,
                compile_js_facade_value_to_binding_expr,
            },
            imports::{Import, ImportFrom},
        },
    },
    frag,
};

/// Compiles an exported facade function that calls its binding export.
///
/// Requires imports added by [`compile_facade_fn_required_imports`] to be in
/// scope.
pub fn compile_facade_fn<'a>(db: &'a dyn Db, fn_: Fn<'a>) -> impl FragmentOnce + 'a {
    let param_conv = fragment(move |p| {
        for param in fn_.params(db).iter() {
            frag!(
                ~{param.binding_name(db)}~ = ~{compile_js_facade_value_to_binding_expr(param.type_(db), param.binding_name(db))};@
            ).apply(p);
        }
    });

    let params = move || comma_list(fn_.params(db).iter().map(|param| param.binding_name(db)));
    let binding_fn_call_expr = frag!({ fn_.bindings_name(db) }({ params() }));

    let body_stmt = fragment(move |p| match fn_.return_type(db) {
        Some(return_type) => {
            frag!(
                return ~{compile_js_binding_value_to_facade_expr(db, return_type, binding_fn_call_expr)};
            ).apply(p);
        }
        None => {
            frag!(
                {binding_fn_call_expr};
            )
            .apply(p);
        }
    });

    frag!(
        export function~{fn_.name(db)}~({params()})~{{
            {param_conv}@
            {body_stmt}
        }}
    )
}

/// Adds imports required by [`compile_facade_fn`].
pub fn compile_facade_fn_required_imports<'db>(
    db: &'db dyn Db,
    fn_: Fn<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    imports.insert(Import {
        item: fn_.bindings_name(db),
        from: ImportFrom::Bindings,
    });

    let active_mod_id = fn_.id(db).get_parent_mod_id(db).unwrap();

    if let Some(return_type) = fn_.return_type(db) {
        compile_js_binding_value_to_facade_expr_required_imports(
            db,
            active_mod_id,
            return_type,
            imports,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        db::DbImpl,
        fragment::{FragmentPrint, spaced},
        js::{
            JsCodegenPaths,
            imports::{ImportFrom, compile_import_stmt},
        },
        rustdoc::fn_::lower_fn,
        test::{assert_snap, test_db},
    };

    fn compile_test_fn(mut db: DbImpl) -> String {
        JsCodegenPaths::set(&mut db, "path/to/bindings".to_owned());

        let fn_ = lower_fn(&db, db.rustdoc_id("foo")).unwrap();
        let mut imports = BTreeSet::new();
        compile_facade_fn_required_imports(&db, fn_, &mut imports);
        let mut imports = imports.into_iter().collect::<Vec<_>>();
        imports.sort_by_key(|import| matches!(import.from, ImportFrom::Bindings));
        let imports = spaced(
            imports
                .into_iter()
                .map(|import| compile_import_stmt(&db, import)),
        );

        frag!(
            {imports}@@
            {compile_facade_fn(&db, fn_)}
        )
        .print()
    }

    #[test]
    fn fn_no_args() {
        assert_snap!({
            @lang: "js";

            let db = test_db! {
                pub fn foo() { }
            };

            compile_test_fn(db)
        });
    }

    #[test]
    fn fn_mixed_args() {
        assert_snap!({
            @lang: "js";

            let db = test_db! {
                #[derive(Clone)]
                pub struct Foo {

                }

                pub fn foo(a: i32, b: String, c: Foo) { }
            };

            compile_test_fn(db)
        });
    }

    #[test]
    fn fn_ret_num() {
        assert_snap!({
            @lang: "js";

            let db = test_db! {
                pub fn foo() -> i32 { 0 }
            };

            compile_test_fn(db)
        });
    }

    #[test]
    fn fn_ret_struct() {
        assert_snap!({
            @lang: "js";

            let db = test_db! {
                #[derive(Clone)]
                pub struct Foo;

                pub fn foo() -> Foo { Foo }
            };

            compile_test_fn(db)
        });
    }

    #[test]
    fn fn_ret_external_struct() {
        assert_snap!({
            @lang: "js";

            let db = test_db! {
                @file: "models.rs";
                pub mod models {
                    #[derive(Clone)]
                    pub struct Foo;
                }

                pub fn foo() -> models::Foo { models::Foo }
            };

            compile_test_fn(db)
        });
    }

    #[test]
    fn fn_mixed_args_ret_struct() {
        assert_snap!({
            @lang: "js";

            let db = test_db! {
                @file: "parent.rs";
                pub mod parent {
                    #[derive(Clone)]
                    pub struct Foo;
                }

                pub fn foo(a: i32, b: String, c: parent::Foo) -> parent::Foo { parent::Foo }
            };

            compile_test_fn(db)
        });
    }
}
