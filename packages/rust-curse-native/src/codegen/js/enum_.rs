use std::collections::BTreeSet;

use crate::{
    codegen::{
        db::Db,
        fragment::{FragmentOnce, comma_list, doubles, fragment, spaced},
        ir::{
            IrId,
            enum_::{Enum, EnumVariant, enum_field_binding_name_from_ir_id, enum_name_from_ir_id},
            shape::ShapeField,
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

#[derive(Clone, Copy)]
struct EnumFieldOccurrence<'db> {
    variant: EnumVariant<'db>,
    variant_index: usize,
    field: ShapeField<'db>,
}

struct EnumFieldGroup<'db> {
    name: &'db str,
    occurrences: Vec<EnumFieldOccurrence<'db>>,
}

/// Compiles a facade JavaScript class for a given enum.
///
/// Requires imports added by [`compile_facade_class_for_enum_required_imports`]
/// to be in scope.
pub fn compile_facade_class_for_enum<'db>(
    db: &'db dyn Db,
    enum_: Enum<'db>,
) -> impl FragmentOnce + 'db {
    fragment(move |p| {
        let groups = group_enum_fields(db, enum_);

        let type_tags_array = frag!([{ comma_list(enum_.js_tag_names(db).iter().map(doubles)) }]);

        let private_cached_fields = fragment(|p| {
            for group in &groups {
                for occurrence in &group.occurrences {
                    if matches!(occurrence.field.type_(db), Type::Struct(_) | Type::Enum(_)) {
                        frag!(#_{occurrence.variant.name(db)}_{occurrence.field.name(db)};@)
                            .apply(p);
                    }
                }
            }
        });

        let pub_fields = spaced(
            groups
                .iter()
                .map(|group| compile_facade_field_group_getter_setter(db, enum_, group)),
        );

        frag!(
            export class~{enum_.name(db)}~{{
                static #_types = {type_tags_array};@
                static #_construction_token = Symbol();@
                #_binding;@
                {private_cached_fields}@

                get ~{enum_.js_tag_field_name(db)}()~{{
                    return ~{enum_.name(db)}.#_types[this.#_binding.__type()];
                }}@@

                {pub_fields}@@

                constructor(token)~{{
                    if (token !== ~{enum_.name(db)}.#_construction_token)~{{
                        throw new TypeError("Rust-backed enums cannot be constructed directly");
                    }}
                }}@@

                static __from_binding(binding)~{{
                    const obj = new ~{enum_.name(db)}(~{enum_.name(db)}.#_construction_token);@
                    obj.#_binding = binding;@
                    return obj;
                }}@@

                __get_binding()~{{
                    return this.#_binding;
                }}
            }}
        )
        .apply(p);
    })
}

/// Adds imports required by [`compile_facade_class_for_enum`].
// todo: deduplicate it somehow with facade class for struct?
pub fn compile_facade_class_for_enum_required_imports<'db>(
    db: &'db dyn Db,
    enum_: Enum<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    let active_mod_id = enum_.id(db).get_parent_mod_id(db).unwrap();

    for variant in enum_.variants(db) {
        for field in variant.fields(db) {
            let field_type = field.type_(db);

            compile_js_binding_value_to_facade_expr_required_imports(
                db,
                active_mod_id,
                field_type,
                imports,
            );

            if !field.is_readonly(db) {
                compile_js_facade_value_validation_stmt_required_imports(
                    db,
                    active_mod_id,
                    field_type,
                    imports,
                );
            }
        }
    }
}

/// Groups enum fields by their name
fn group_enum_fields<'db>(db: &'db dyn Db, enum_: Enum<'db>) -> Vec<EnumFieldGroup<'db>> {
    let mut groups = Vec::<EnumFieldGroup<'db>>::new();

    for (variant_index, variant) in enum_.variants(db).iter().copied().enumerate() {
        for field in variant.fields(db) {
            let name = field.name(db).as_str();
            let occurrence = EnumFieldOccurrence {
                variant,
                variant_index,
                field,
            };

            if let Some(group) = groups.iter_mut().find(|group| group.name == name) {
                group.occurrences.push(occurrence);
            } else {
                groups.push(EnumFieldGroup {
                    name,
                    occurrences: vec![occurrence],
                });
            }
        }
    }

    groups
}

fn compile_facade_field_group_getter_setter<'a, 'db: 'a>(
    db: &'db dyn Db,
    enum_: Enum<'db>,
    group: &'a EnumFieldGroup<'db>,
) -> impl FragmentOnce + 'a {
    fragment(move |p| {
        compile_field_group_getter(db, enum_, group).apply(p);

        if group
            .occurrences
            .iter()
            .any(|occurrence| !occurrence.field.is_readonly(db))
        {
            frag!(@@{compile_field_group_setter(db, enum_, group)}).apply(p);
        }
    })
}

fn compile_field_group_getter<'a, 'db: 'a>(
    db: &'db dyn Db,
    enum_: Enum<'db>,
    group: &'a EnumFieldGroup<'db>,
) -> impl FragmentOnce + 'a {
    fragment(move |p| {
        let accessor = compile_public_field_accessor(group.name);

        match group.occurrences.as_slice() {
            [occurrence] => {
                frag!(
                    get ~{accessor}() {{
                        {compile_field_read_stmt(db, *occurrence)}
                    }}
                )
                .apply(p);
            }
            occurrences => {
                let branches = fragment(move |p| {
                    for occurrence in occurrences.iter().copied() {
                        frag!(
                            if (type_ === ~{occurrence.variant_index}) {{
                                {compile_field_read_stmt(db, occurrence)}
                            }}@
                        )
                        .apply(p);
                    }
                });

                frag!(
                    get ~{accessor}() {{
                        const type_ = this.#_binding.__type();@
                        {branches}
                        {compile_unavailable_field_error(db, enum_, group.name)}
                    }}
                )
                .apply(p);
            }
        }
    })
}

fn compile_field_group_setter<'a, 'db: 'a>(
    db: &'db dyn Db,
    enum_: Enum<'db>,
    group: &'a EnumFieldGroup<'db>,
) -> impl FragmentOnce + 'a {
    fragment(move |p| {
        let accessor = compile_public_field_accessor(group.name);

        match group.occurrences.as_slice() {
            [occurrence] => {
                frag!(
                    set ~{accessor}(value) {{
                        {compile_field_assignment_stmt(db, *occurrence)}
                    }}
                )
                .apply(p);
            }
            occurrences => {
                let branches = fragment(move |p| {
                    for occurrence in occurrences.iter().copied() {
                        if occurrence.field.is_readonly(db) {
                            frag!(
                                if (type_ === ~{occurrence.variant_index}) {{
                                    throw new TypeError("Field is readonly on the active enum variant");
                                }}@
                            )
                            .apply(p);
                        } else {
                            frag!(
                                if (type_ === ~{occurrence.variant_index}) {{
                                    {compile_field_assignment_stmt(db, occurrence)}@
                                    return;
                                }}@
                            )
                            .apply(p);
                        }
                    }
                });

                frag!(
                    set ~{accessor}(value) {{
                        const type_ = this.#_binding.__type();@
                        {branches}
                        {compile_unavailable_field_error(db, enum_, group.name)}
                    }}
                )
                .apply(p);
            }
        }
    })
}

fn compile_field_read_stmt<'db>(
    db: &'db dyn Db,
    occurrence: EnumFieldOccurrence<'db>,
) -> impl FragmentOnce + 'db {
    let field_type = occurrence.field.type_(db);

    fragment(move |p| match field_type {
        Type::Struct(_) | Type::Enum(_) => {
            frag!(
                const binding = ~{compile_enum_binding_field_expr(db, occurrence)};@
                return ~{compile_cached_facade_field_expr(db, occurrence)}~ ??= ~{compile_js_binding_value_to_facade_expr(
                    db,
                    field_type,
                    frag!(binding),
                )};
            )
            .apply(p);
        }
        Type::Primitive(_) | Type::Str | Type::String => {
            frag!(
                return ~{compile_js_binding_value_to_facade_expr(
                    db,
                    field_type,
                    compile_enum_binding_field_expr(db, occurrence),
                )};
            )
            .apply(p);
        }
    })
}

fn compile_field_assignment_stmt<'db>(
    db: &'db dyn Db,
    occurrence: EnumFieldOccurrence<'db>,
) -> impl FragmentOnce + 'db {
    let field_type = occurrence.field.type_(db);
    let has_cached_facade = matches!(field_type, Type::Struct(_) | Type::Enum(_));

    fragment(move |p| {
        if has_cached_facade {
            frag!(
                {compile_js_facade_value_validation_stmt(
                    db,
                    field_type,
                    frag!(value),
                )}@
            )
            .apply(p);
        }

        frag!(
            {compile_enum_binding_field_expr(db, occurrence)}~ = ~{compile_js_facade_value_to_binding_expr(
                field_type,
                frag!(value),
            )};
        )
        .apply(p);

        if has_cached_facade {
            frag!(@
                {compile_cached_facade_field_expr(db, occurrence)}~ = ~{compile_js_binding_value_to_facade_expr(
                    db,
                    field_type,
                    compile_enum_binding_field_expr(db, occurrence),
                )};
            )
            .apply(p);
        }
    })
}

// todo: make it as utility for overall js codegen and reuse in structs as well
fn compile_public_field_accessor(name: &str) -> impl FragmentOnce + '_ {
    fragment(move |p| {
        if name.bytes().all(|byte| byte.is_ascii_digit()) {
            frag!([{ name }]).apply(p);
        } else {
            frag!([{ doubles(name) }]).apply(p);
        }
    })
}

fn compile_cached_facade_field_expr<'db>(
    db: &'db dyn Db,
    occurrence: EnumFieldOccurrence<'db>,
) -> impl FragmentOnce + 'db {
    frag!(this.#_{occurrence.variant.name(db)}_{occurrence.field.name(db)})
}

fn compile_enum_binding_field_expr<'db>(
    db: &'db dyn Db,
    occurrence: EnumFieldOccurrence<'db>,
) -> impl FragmentOnce + 'db {
    let binding_name = enum_field_binding_name_from_ir_id(db, occurrence.variant, occurrence.field);
    frag!(this.#_binding[{doubles(binding_name)}])
}

fn compile_unavailable_field_error<'db>(
    db: &'db dyn Db,
    enum_: Enum<'db>,
    name: &'db str,
) -> impl FragmentOnce + 'db {
    frag!(
        throw new TypeError({doubles((
            "Field ",
            name,
            " is not available on the active ",
            enum_.name(db),
            " variant",
        ))});
    )
}

/// Compiles an expression that wraps a binding class in its enum facade class.
///
/// Requires imports added by
/// [`compile_facade_class_from_binding_expr_required_imports`] to be in scope.
pub fn compile_facade_class_from_binding_expr<'db>(
    db: &'db dyn Db,
    enum_id: IrId<'db>,
    binding_class_expr: impl FragmentOnce,
) -> impl FragmentOnce {
    frag!({ enum_name_from_ir_id(db, enum_id) }.__from_binding({ binding_class_expr }))
}

/// Adds imports required by [`compile_facade_class_from_binding_expr`].
pub fn compile_facade_class_from_binding_expr_required_imports<'db>(
    db: &'db dyn Db,
    active_mod_id: IrId<'db>,
    enum_id: IrId<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    let enum_mod_id = enum_id.get_parent_mod_id(db).unwrap();

    if active_mod_id == enum_mod_id {
        return;
    }

    imports.insert(Import {
        item: enum_name_from_ir_id(db, enum_id),
        from: ImportFrom::RelativeToMod {
            from: active_mod_id,
            to: enum_mod_id,
        },
    });
}

/// Compiles an expression that unwraps an enum facade class into its binding
/// class.
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
        rustdoc::enum_::{LoweredEnum, lower_enum},
        test::{assert_snap, test_db},
    };

    fn compile_test_enum(db: &DbImpl, enum_name: &str) -> String {
        let enum_ = match lower_enum(db, db.rustdoc_id(enum_name)).unwrap() {
            LoweredEnum::Enum(enum_) => enum_,
            LoweredEnum::CLikeEnum(_) => panic!("test requires a data-carrying enum"),
        };
        let mut imports = BTreeSet::new();
        compile_facade_class_for_enum_required_imports(db, enum_, &mut imports);
        let imports = spaced(
            imports
                .into_iter()
                .map(|import| compile_import_stmt(db, import)),
        );

        frag!(
            {imports}@@
            {compile_facade_class_for_enum(db, enum_)}
        )
        .print()
    }

    #[test]
    fn enum_with_tuple_variant() {
        assert_snap!({
            @lang: "js";

            let mut db = test_db! {
                #[derive(Clone)]
                pub struct Point {
                    pub x: f64,
                    pub y: f64,
                }

                pub enum Example {
                    Tuple(Point, String, i32),
                }
            };

            JsCodegenPaths::set(&mut db, "path/to/bindings".to_owned());
            compile_test_enum(&db, "Example")
        });
    }

    #[test]
    fn enum_with_struct_variant() {
        assert_snap!({
            @lang: "js";

            let mut db = test_db! {
                #[derive(Clone)]
                pub struct Point {
                    pub x: f64,
                    pub y: f64,
                }

                pub enum Example {
                    Struct {
                        point: Point,
                        label: String,
                        count: i32,
                    },
                }
            };

            JsCodegenPaths::set(&mut db, "path/to/bindings".to_owned());
            compile_test_enum(&db, "Example")
        });
    }

    #[test]
    fn mixed_enum() {
        assert_snap!({
            @lang: "js";

            let mut db = test_db! {
                #[derive(Clone)]
                pub struct Point {
                    pub x: f64,
                    pub y: f64,
                }

                pub enum Example {
                    Unit,
                    Tuple(Point, String, i32),
                    Struct {
                        point: Point,
                        label: String,
                        count: i32,
                    },
                }
            };

            JsCodegenPaths::set(&mut db, "path/to/bindings".to_owned());
            compile_test_enum(&db, "Example")
        });
    }

    #[test]
    fn shared_fields() {
        assert_snap!({
            @lang: "js";

            let mut db = test_db! {
                @file: "models.rs";
                pub mod models {
                    #[derive(Clone)]
                    pub struct ExternalPoint;
                }

                #[derive(Clone)]
                pub struct Point {
                    pub x: f64,
                    pub y: f64,
                }

                pub struct Readonly;

                pub enum Example {
                    First {
                        value: models::ExternalPoint,
                        mixed: Point,
                        locked: Readonly,
                    },
                    Second {
                        value: String,
                        mixed: Readonly,
                        locked: Readonly,
                    },
                    Third,
                }
            };

            JsCodegenPaths::set(&mut db, "path/to/bindings".to_owned());
            compile_test_enum(&db, "Example")
        });
    }

    #[test]
    fn type_field_renames_discriminant() {
        assert_snap!({
            @lang: "js";

            let mut db = test_db! {
                pub enum Example {
                    Value { r#type: String },
                }
            };

            JsCodegenPaths::set(&mut db, "path/to/bindings".to_owned());
            compile_test_enum(&db, "Example")
        });
    }
}
