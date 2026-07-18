use std::collections::BTreeSet;

use crate::{
    codegen::{
        db::Db,
        fragment::{FragmentOnce, doubles, fragment, spaced},
        ir::{
            IrId,
            shape::Shape,
            struct_::{Struct, struct_name_from_ir_id},
            type_::Type,
        },
        js::{
            boundary::{
                compile_js_binding_value_to_facade_expr,
                compile_js_binding_value_to_facade_expr_required_imports,
                compile_js_facade_value_to_binding_expr,
            },
            imports::{Import, ImportFrom},
            validation::{
                compile_js_facade_value_validation_stmt,
                compile_js_facade_value_validation_stmt_required_imports,
            },
        },
    },
    frag,
};

/// Compiles a facade JavaScript class for a given struct.
///
/// Requires imports added by
/// [`compile_facade_class_for_struct_required_imports`] to be in scope.
pub fn compile_facade_class_for_struct<'a>(
    db: &'a dyn Db,
    struct_: Struct<'a>,
) -> impl FragmentOnce + 'a {
    let shape = struct_.shape(db);

    let private_fields = fragment(move |p| {
        frag!(
            #_binding;@
            static #_construction_token = Symbol();@
        )
        .apply(p);

        for field in struct_
            .fields(db)
            .iter()
            .copied()
            .filter(|field| match field.type_(db) {
                Type::Struct(_) | Type::Enum(_) => true,
                Type::Primitive(_) | Type::Str | Type::String => false,
            })
        {
            frag!(#_{field.name(db)};@).apply(p);
        }
    });

    let fields = spaced(struct_.fields(db).iter().copied().map(move |field| {
        let field_name = fragment(move |p| match shape {
            Shape::Tuple(_) => field.name(db).apply(p),
            Shape::Plain { .. } => doubles(field.name(db)).apply(p),
            Shape::Unit => unreachable!(),
        });
        let cached_field_expr = frag!(this.#_{field.name(db)});
        let binding_field_name = field_name.clone();
        let binding_field_expr = move || frag!(this.#_binding[{binding_field_name.clone()}]);

        fragment(move |p| {
            match field.type_(db) {
                Type::Struct(_) | Type::Enum(_) => {
                    frag!(
                        get ~[{field_name.clone()}]() {{
                            return ~{cached_field_expr};
                        }}
                    )
                    .apply(p);
                }
                Type::Primitive(_) | Type::Str | Type::String => {
                    frag!(
                        get ~[{field_name.clone()}]() {{
                            return ~{compile_js_binding_value_to_facade_expr(
                                db,
                                field.type_(db),
                                binding_field_expr(),
                            )};
                        }}
                    )
                    .apply(p);
                }
            }

            if field.is_readonly(db) {
                return;
            }

            match field.type_(db) {
                Type::Struct(_) | Type::Enum(_) => {
                    frag!(@@
                        set ~[{field_name.clone()}](value) {{
                            {compile_js_facade_value_validation_stmt(
                                db,
                                field.type_(db),
                                frag!(value),
                            )}@
                            {binding_field_expr()}~ = ~{compile_js_facade_value_to_binding_expr(
                                field.type_(db),
                                frag!(value),
                            )};@
                            {cached_field_expr}~ = ~{compile_js_binding_value_to_facade_expr(
                                db,
                                field.type_(db),
                                binding_field_expr(),
                            )};
                        }}
                    )
                    .apply(p);
                }
                Type::Primitive(_) | Type::Str | Type::String => {
                    frag!(@@
                        set ~[{field_name}](value) {{
                            {binding_field_expr()}~ = ~{compile_js_facade_value_to_binding_expr(
                                field.type_(db),
                                frag!(value),
                            )};
                        }}
                    )
                    .apply(p);
                }
            }
        })
    }));

    let initialize_cached_fields = fragment(move |p| {
        for field in struct_
            .fields(db)
            .iter()
            .filter(|field| match field.type_(db) {
                Type::Struct(_) | Type::Enum(_) => true,
                Type::Primitive(_) | Type::Str | Type::String => false,
            })
        {
            let field_name = fragment(|p| match shape {
                Shape::Tuple(_) => field.name(db).apply(p),
                Shape::Plain { .. } => doubles(field.name(db)).apply(p),
                Shape::Unit => unreachable!(),
            });
            let cached_field_expr = frag!(obj.#_{field.name(db)});

            frag!(
                {cached_field_expr}~ = ~{compile_js_binding_value_to_facade_expr(
                    db,
                    field.type_(db),
                    frag!(binding[{field_name}]),
                )};@
            )
            .apply(p);
        }
    });

    frag!(
        export class~{struct_.name(db)}~{{
            {private_fields}@

            {fields}@@

            constructor(token) {{
                if (token !== ~{struct_.name(db)}.#_construction_token) {{
                    throw new TypeError("Rust-backed structs cannot be constructed directly");
                }}
            }}@@

            static __from_binding(binding) {{
                const obj = new ~{struct_.name(db)}(~{struct_.name(db)}.#_construction_token);@
                obj.#_binding = binding;@
                {initialize_cached_fields}
                return obj;
            }}@@

            __get_binding() {{
                return this.#_binding;
            }}
        }}
    )
}

/// Adds imports required by [`compile_facade_class_for_struct`].
pub fn compile_facade_class_for_struct_required_imports<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    let active_mod_id = struct_.id(db).get_parent_mod_id(db).unwrap();

    for field in struct_.fields(db).iter() {
        compile_js_binding_value_to_facade_expr_required_imports(
            db,
            active_mod_id,
            field.type_(db),
            imports,
        );

        if !field.is_readonly(db) {
            compile_js_facade_value_validation_stmt_required_imports(
                db,
                active_mod_id,
                field.type_(db),
                imports,
            );
        }
    }
}

/// Compiles an expression that wraps a binding class in its facade class.
///
/// Requires imports added by
/// [`compile_facade_class_from_binding_expr_required_imports`] to be in scope.
pub fn compile_facade_class_from_binding_expr<'db>(
    db: &'db dyn Db,
    struct_id: IrId<'db>,
    binding_class_expr: impl FragmentOnce,
) -> impl FragmentOnce {
    frag!({ struct_name_from_ir_id(db, struct_id) }.__from_binding({ binding_class_expr }))
}

/// Adds imports required by [`compile_facade_class_from_binding_expr`].
pub fn compile_facade_class_from_binding_expr_required_imports<'db>(
    db: &'db dyn Db,
    active_mod_id: IrId<'db>,
    struct_id: IrId<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    let struct_mod_id = struct_id.get_parent_mod_id(db).unwrap();

    if active_mod_id == struct_mod_id {
        return;
    }

    imports.insert(Import {
        item: struct_name_from_ir_id(db, struct_id),
        from: ImportFrom::RelativeToMod {
            from: active_mod_id,
            to: struct_mod_id,
        },
    });
}

/// Compiles an expression that unwraps a facade class into its binding class.
pub fn compile_binding_class_from_facade_expr(
    facade_class_expr: impl FragmentOnce,
) -> impl FragmentOnce {
    frag!({ facade_class_expr }.__get_binding())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        db::DbImpl,
        fragment::{FragmentPrint, spaced},
        js::{JsCodegenPaths, imports::compile_import_stmt},
        rustdoc::struct_::lower_struct,
        test::{assert_snap, test_db},
    };

    fn compile_test_struct(db: &DbImpl, struct_name: &str) -> String {
        let struct_ = lower_struct(db, db.rustdoc_id(struct_name)).unwrap();
        let mut imports = BTreeSet::new();
        compile_facade_class_for_struct_required_imports(db, struct_, &mut imports);
        let imports = spaced(
            imports
                .into_iter()
                .map(|import| compile_import_stmt(db, import)),
        );

        frag!(
            {imports}@@
            {compile_facade_class_for_struct(db, struct_)}
        )
        .print()
    }

    #[test]
    fn class_from_plain_struct() {
        assert_snap!({
            @lang: "js";

            let mut db = test_db! {
                @file: "baz.rs";
                pub mod baz {
                    #[derive(Clone)]
                    pub struct Baz {}
                }

                pub struct Qux {}

                pub struct Foo {
                    pub x: i32,
                    pub y: String,
                    pub z: baz::Baz,
                    pub v: Qux,
                }
            };

            JsCodegenPaths::set(&mut db, "path/to/bindings".to_owned());
            compile_test_struct(&db, "Foo")
        });
    }

    #[test]
    fn class_from_tuple_struct() {
        assert_snap!({
            @lang: "js";

            let mut db = test_db! {
                #[derive(Clone)]
                pub struct Baz {}
                pub struct Qux {}

                #[derive(Clone)]
                pub struct Foo(pub Baz, pub Qux, pub String);
            };

            JsCodegenPaths::set(&mut db, "path/to/bindings".to_owned());
            compile_test_struct(&db, "Foo")
        });
    }

    #[test]
    fn class_from_unit_struct() {
        assert_snap!({
            @lang: "js";

            let mut db = test_db! {
                pub struct Foo;
                pub struct Bar();
                pub struct Baz {}
            };
            JsCodegenPaths::set(&mut db, "path/to/bindings".to_owned());

            frag!(
                {compile_test_struct(&db, "Foo")}@@
                {compile_test_struct(&db, "Bar")}@@
                {compile_test_struct(&db, "Baz")}
            )
            .print()
        });
    }
}
