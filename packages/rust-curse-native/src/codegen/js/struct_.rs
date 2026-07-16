use std::collections::BTreeSet;

use crate::{
    codegen::{
        db::Db,
        fragment::{FragmentOnce, doubles, fragment, spaced},
        ir::{
            IrId,
            struct_::{Struct, StructShape, struct_name_from_rustdoc_id},
            type_::Type,
        },
        js::{
            boundary::{
                compile_js_boundary_in_to_facade_type_expr,
                compile_js_boundary_out_from_facade_type_expr,
                compile_js_boundary_out_from_facade_type_expr_required_imports,
            },
            imports::{Import, ImportFrom},
        },
    },
    frag,
};

/// Compiles a facade JavaScript class for a given struct.
///
/// Requires [`compile_facade_class_for_struct_required_imports`] imports to be
/// in scope.
pub fn compile_facade_class_for_struct<'a>(
    db: &'a dyn Db,
    struct_: Struct<'a>,
) -> impl FragmentOnce + 'a {
    let shape = struct_.shape(db);

    let private_fields = fragment(move |p| {
        frag!(#_binding;@).apply(p);

        for field in struct_
            .fields(db)
            .iter()
            .copied()
            .filter(|field| matches!(field.type_(db), Type::Struct(_)))
        {
            frag!(#_{field.name(db)};@).apply(p);
        }
    });

    let fields = spaced(struct_.fields(db).iter().copied().map(move |field| {
        let field_name = fragment(move |p| match shape {
            StructShape::Tuple(_) => field.name(db).apply(p),
            StructShape::Plain { .. } => doubles(field.name(db)).apply(p),
            StructShape::Unit => unreachable!(),
        });
        let cached_field_expr = frag!(this.#_{field.name(db)});
        let binding_field_name = field_name.clone();
        let binding_field_expr = move || frag!(this.#_binding[{binding_field_name.clone()}]);

        fragment(move |p| {
            match field.type_(db) {
                Type::Struct(_) => {
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
                            return ~{compile_js_boundary_out_from_facade_type_expr(
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
                Type::Struct(_) => {
                    frag!(@@
                        set ~[{field_name.clone()}](value) {{
                            {binding_field_expr()}~ = ~{compile_js_boundary_out_from_facade_type_expr(
                                field.type_(db),
                                frag!(value),
                            )};@
                            {cached_field_expr}~ = ~{compile_js_boundary_in_to_facade_type_expr(
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
                            {binding_field_expr()}~ = ~{compile_js_boundary_out_from_facade_type_expr(
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
            .filter(|field| matches!(field.type_(db), Type::Struct(_)))
        {
            let field_name = fragment(|p| match shape {
                StructShape::Tuple(_) => field.name(db).apply(p),
                StructShape::Plain { .. } => doubles(field.name(db)).apply(p),
                StructShape::Unit => unreachable!(),
            });
            let cached_field_expr = frag!(obj.#_{field.name(db)});

            frag!(
                {cached_field_expr}~ = ~{compile_js_boundary_in_to_facade_type_expr(
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

            static __from_binding(binding) {{
                const obj = new ~{struct_.name(db)}();@
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

/// Returns the imports required by [`compile_facade_class_for_struct`].
pub fn compile_facade_class_for_struct_required_imports<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    let active_mod_id = struct_.id(db).get_parent_mod_id(db).unwrap();

    for field in struct_.fields(db).iter() {
        compile_js_boundary_out_from_facade_type_expr_required_imports(
            db,
            active_mod_id,
            field.type_(db),
            imports,
        );
    }
}

/// Compiles an expression that creates a new facade class from an
/// expression evaluating to a binding class value.
///
/// Requires to [`compile_binding_class_to_facade_class_required_imports`]
/// imports to be in scope.
pub fn compile_binding_class_to_facade_class_expr<'db>(
    db: &'db dyn Db,
    struct_id: IrId<'db>,
    binding_class_expr: impl FragmentOnce,
) -> impl FragmentOnce {
    frag!(
        { struct_name_from_rustdoc_id(db, struct_id.rustdoc_id(db)) }
            .__from_binding({ binding_class_expr })
    )
}

/// Compiles the required imports for a
/// [`compile_binding_class_to_facade_class_expr`] call.
pub fn compile_binding_class_to_facade_class_required_imports<'db>(
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
        item: struct_name_from_rustdoc_id(db, struct_id.rustdoc_id(db)),
        from: ImportFrom::RelativeToMod {
            from: active_mod_id,
            to: struct_mod_id,
        },
    });
}

/// Compiles an expression that gets a binding class from an expression
/// evaluating to a facade class value.
pub fn compile_binding_class_from_facade_class_expr(
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

    fn compile_test_struct(mut db: DbImpl, struct_name: &str) -> String {
        JsCodegenPaths::set(&mut db, "path/to/bindings".to_owned());

        let struct_ = lower_struct(&db, db.rustdoc_id(struct_name)).unwrap();
        let mut imports = BTreeSet::new();
        compile_facade_class_for_struct_required_imports(&db, struct_, &mut imports);
        let imports = spaced(
            imports
                .into_iter()
                .map(|import| compile_import_stmt(&db, import)),
        );

        frag!(
            {imports}@@
            {compile_facade_class_for_struct(&db, struct_)}
        )
        .print()
    }

    #[test]
    fn class_from_plain_struct() {
        assert_snap!({
            @lang: "js";

            let db = test_db! {
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

            compile_test_struct(db, "Foo")
        });
    }

    #[test]
    fn class_from_tuple_struct() {
        assert_snap!({
            @lang: "js";

            let db = test_db! {
                #[derive(Clone)]
                pub struct Baz {}
                pub struct Qux {}

                #[derive(Clone)]
                pub struct Foo(pub Baz, pub Qux, pub String);
            };

            compile_test_struct(db, "Foo")
        });
    }

    #[test]
    fn class_from_unit_struct() {
        assert_snap!({
            @lang: "js";

            let db = test_db! {
                pub struct Foo;
                pub struct Bar();
                pub struct Baz {}
            };

            frag!(
                {compile_test_struct(db.clone(), "Foo")}@@
                {compile_test_struct(db.clone(), "Bar")}@@
                {compile_test_struct(db, "Baz")}
            )
            .print()
        });
    }
}
