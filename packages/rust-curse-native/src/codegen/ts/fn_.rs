use std::collections::BTreeSet;

use crate::{
    codegen::{
        db::Db,
        fragment::{FragmentOnce, FragmentPrint, IntoFnFragment, Printer, comma_list},
        ir::{fn_::Fn, type_::Type},
        js::imports::{Import, ImportFrom},
        ts::jsdoc::compile_jsdoc,
    },
    frag,
};

/// Compiles an IR function into a TypeScript function declaration.
///
/// Requires imports added by [`compile_fn_required_imports`] to be in scope.
#[salsa::tracked(returns(ref))]
pub fn compile_fn<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> String {
    let params = (move |printer: &mut Printer| {
        if fn_.params(db).is_empty() {
            "()".apply(printer);
        } else {
            frag!(({
                comma_list(
                    fn_.params(db)
                        .iter()
                        .map(|param| frag!({param.name(db)}:~{param.type_(db).ts_name(db)})),
                )
            },))
            .apply(printer);
        }
    })
    .into_fragment();
    let return_type = match fn_.return_type(db) {
        Some(type_) => &type_.ts_name(db),
        None => "void",
    };

    frag!(
        {compile_jsdoc(fn_.docs(db).as_deref())}
        export function ~{fn_.name(db)}{params}:~{return_type};
    )
    .print()
}

/// Adds imports required by [`compile_fn`].
pub fn compile_fn_required_imports<'db>(
    db: &'db dyn Db,
    fn_: Fn<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    let active_mod_id = fn_.id(db).parent_mod_id(db).unwrap();

    for type_ in fn_
        .params(db)
        .iter()
        .map(|param| param.type_(db))
        .chain(fn_.return_type(db))
    {
        let (type_id, type_name) = match type_ {
            Type::Struct(struct_id) => (struct_id.inner(db), struct_id.name(db)),
            Type::Enum(enum_id) => (enum_id.inner(db), enum_id.name(db)),
            Type::Primitive(_) | Type::Str | Type::String => continue,
        };
        let type_mod_id = type_id.parent_mod_id(db).unwrap();

        if active_mod_id != type_mod_id {
            imports.insert(Import {
                item: type_name,
                from: ImportFrom::RelativeToMod {
                    from: active_mod_id,
                    to: type_mod_id,
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        db::DbImpl,
        fragment::{FragmentPrint, spaced},
        js::imports::compile_import_stmt,
        rustdoc::fn_::lower_fn,
        test::{assert_snap, test_db},
    };

    fn compile_test_fn(db: DbImpl) -> String {
        let fn_ = lower_fn(&db, db.rustdoc_id("foo")).unwrap();
        let mut imports = BTreeSet::new();
        compile_fn_required_imports(&db, fn_, &mut imports);

        if imports.is_empty() {
            compile_fn(&db, fn_).to_string()
        } else {
            frag!(
                {spaced(imports.into_iter().map(|import| compile_import_stmt(&db, import)))}@@
                {compile_fn(&db, fn_)}
            )
            .print()
        }
    }

    #[test]
    fn fn_no_args() {
        assert_snap!({
            @lang: "ts";

            let db = test_db! {
                pub fn foo() {}
            };

            compile_test_fn(db)
        });
    }

    #[test]
    fn fn_mixed_args() {
        assert_snap!({
            @lang: "ts";

            let db = test_db! {
                #[derive(Clone)]
                pub struct Local;

                @file: "models.rs";
                pub mod models {
                    #[derive(Clone)]
                    pub struct External;
                }

                /// Accepts primitive, local, and external values.
                pub fn foo(number: i32, text: String, local: Local, external: models::External) {}
            };

            compile_test_fn(db)
        });
    }

    #[test]
    fn fn_ret_num() {
        assert_snap!({
            @lang: "ts";

            let db = test_db! {
                pub fn foo() -> i32 { 0 }
            };

            compile_test_fn(db)
        });
    }

    #[test]
    fn fn_ret_struct() {
        assert_snap!({
            @lang: "ts";

            let db = test_db! {
                pub struct Foo;

                pub fn foo() -> Foo { Foo }
            };

            compile_test_fn(db)
        });
    }

    #[test]
    fn fn_ret_external_struct() {
        assert_snap!({
            @lang: "ts";

            let db = test_db! {
                @file: "models.rs";
                pub mod models {
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
            @lang: "ts";

            let db = test_db! {
                @file: "parent.rs";
                pub mod parent {
                    #[derive(Clone)]
                    pub struct Foo;
                }

                pub fn foo(number: i32, text: String, value: parent::Foo) -> parent::Foo {
                    value
                }
            };

            compile_test_fn(db)
        });
    }
}
