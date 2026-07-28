//! Module for compiling enums into binding structs that can be exposed to
//! JavaScript.

use crate::{
    codegen::{
        api::{Api, ApiKind},
        bindings::{
            boundary::{
                BoundaryInExpr, BoundaryMode, compile_boundary_in_to_source_type_expr,
                compile_boundary_in_type, compile_boundary_out_from_source_type_expr,
                compile_boundary_out_from_tethered_struct_expr, compile_boundary_out_type,
            },
            tethering::{
                ClonedField, TetheredField, TetheredStructField, TetheringStruct,
                compile_tethered_struct, compile_tethered_struct_get_field_expr,
                compile_tethered_struct_set_field_stmt,
            },
        },
        db::Db,
        fragment::{FragmentOnce, FragmentPrint, doubles, fragment, spaced},
        ir::{
            enum_::{Enum, EnumId, EnumVariant},
            item::ItemId,
            shape::{Shape, ShapeField},
            type_::Type,
        },
    },
    frag,
};

impl<'db> EnumId<'db> {
    /// Returns the binding mangled name, e.g.
    /// `__binding_enum__core__option__Option`.
    pub fn binding_name(self, db: &'db dyn Db) -> &'db String {
        binding_name(db, self.into())
    }

    /// Returns the paired tethering struct name.
    pub fn tethering_struct_name(self, db: &'db dyn Db) -> &'db String {
        tethering_struct_name(db, self.into())
    }
}

#[salsa::tracked(returns(ref))]
fn binding_name<'db>(db: &'db dyn Db, id: ItemId<'db>) -> String {
    format!("__binding_enum__{}", id.mangled_fqn(db))
}

#[salsa::tracked(returns(ref))]
fn tethering_struct_name<'db>(db: &'db dyn Db, id: ItemId<'db>) -> String {
    format!("__tethered_struct__{}", id.mangled_fqn(db))
}

impl<'db> Enum<'db> {
    /// Returns the name of the binding struct for this enum.
    pub fn binding_name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).binding_name(db)
    }

    /// Paired tethering struct name, e.g.
    /// `__tethered_struct__core__option__Option`.
    pub fn tethering_struct_name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).tethering_struct_name(db)
    }
}

// todo: move to variant?
#[salsa::tracked]
impl<'db> ShapeField<'db> {
    /// Returns the enum variant field binding getter function name, e.g.
    /// `__getter__Circle__radius`.
    #[salsa::tracked(returns(ref))]
    pub fn getter_binding_name_on_enum(self, db: &'db dyn Db, variant: EnumVariant<'db>) -> String {
        format!("__getter{}", self.enum_binding_name(db, variant))
    }

    /// Returns the enum variant field binding setter function name, e.g.
    /// `__setter__Circle__radius`.
    #[salsa::tracked(returns(ref))]
    pub fn setter_binding_name_on_enum(self, db: &'db dyn Db, variant: EnumVariant<'db>) -> String {
        format!("__setter{}", self.enum_binding_name(db, variant))
    }

    /// Binding mangled name for a field, e.g. `__Circle__radius`.
    #[salsa::tracked(returns(ref))]
    pub fn enum_binding_name(self, db: &'db dyn Db, variant: EnumVariant<'db>) -> String {
        format!("__{}__{}", variant.name(db), self.name(db))
    }
}

/// Compiles a Rust enum into its tethered backing struct and public binding
/// wrapper.
#[salsa::tracked(returns(ref))]
pub fn compile_binding_enum<'db>(db: &'db dyn Db, enum_: Enum<'db>) -> String {
    let tethered_fields = enum_.variants(db).iter().copied().flat_map(move |variant| {
        variant.fields(db).map(move |field| {
            let name = field.enum_binding_name(db, variant);
            let type_ = field.type_(db);
            let get_expr = move |source: &'static str| {
                compile_variant_field_access_expr(db, enum_, variant, field, source)
            };

            match type_ {
                Type::Struct(id) => {
                    let tethered_name = id.tethering_struct_name(db);

                    TetheredField::TetheredStruct(TetheredStructField {
                        name,
                        get_ref_expr: get_expr,
                        get_mut_expr: get_expr,
                        source_type: type_.fqn(db),
                        tethered_name,
                        with_setter: !field.is_readonly(db),
                    })
                }
                Type::Enum(id) => {
                    let tethered_name = id.tethering_struct_name(db);

                    TetheredField::TetheredStruct(TetheredStructField {
                        name,
                        get_ref_expr: get_expr,
                        get_mut_expr: get_expr,
                        source_type: type_.fqn(db),
                        tethered_name,
                        with_setter: !field.is_readonly(db),
                    })
                }
                Type::Primitive(_) | Type::Str | Type::String => {
                    TetheredField::Cloned(ClonedField {
                        name,
                        get_ref_expr: get_expr,
                        get_mut_expr: get_expr,
                        source_type: type_.fqn(db),
                    })
                }
            }
        })
    });

    frag!(
        {compile_tethered_struct(TetheringStruct {
            source_type: enum_.fqn(db),
            name: enum_.tethering_struct_name(db),
            fields: tethered_fields,
        })}@@
        {compile_wrapper_struct(db, enum_)}@@
        {compile_wrapper_impl(db, enum_)}
    )
    .print()
}

fn compile_wrapper_struct<'db>(db: &'db dyn Db, enum_: Enum<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);
    let header = match api.kind(db) {
        ApiKind::Wasm { .. } => "#[wasm_bindgen(js_name = ",
        ApiKind::Node => "#[napi(js_name = ",
    };
    let binding_name = enum_.binding_name(db);

    frag!(
        {header}{doubles(binding_name)}{")]"}
        pub struct ~{binding_name}~{{
            src: std::rc::Rc<{enum_.tethering_struct_name(db)}>
        }}
    )
}

fn compile_wrapper_impl<'db>(db: &'db dyn Db, enum_: Enum<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);
    let fields = spaced(enum_.variants(db).iter().copied().flat_map(move |variant| {
        variant.fields(db).map(move |field| {
            fragment(move |printer| {
                frag!({compile_field_getter(db, variant, field)}@).apply(printer);

                if !field.is_readonly(db) {
                    frag!(@{compile_field_setter(db, variant, field)}).apply(printer);
                }
            })
        })
    }));

    frag!(
        {api.impl_header(db)}@
        impl~{enum_.binding_name(db)}~{{
            {compile_type_method(db, enum_)}@@

            {fields}
        }}
    )
}

fn compile_type_method<'db>(db: &'db dyn Db, enum_: Enum<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);
    let header = match api.kind(db) {
        ApiKind::Wasm { .. } => "#[wasm_bindgen(js_name = \"__type\")]",
        ApiKind::Node => "#[napi(js_name = \"__type\")]",
    };
    let variants = fragment(move |printer| {
        for (index, variant) in enum_.variants(db).iter().enumerate() {
            let variant_name = variant.name(db);

            match variant.shape(db) {
                Shape::Unit => {
                    frag!({enum_.fqn(db)}::{variant_name}~ => {index}u32,@).apply(printer);
                }
                Shape::Tuple(_) => {
                    frag!({enum_.fqn(db)}::{variant_name}(..) => {index}u32,@).apply(printer);
                }
                Shape::Plain { .. } => {
                    frag!({enum_.fqn(db)}::{variant_name}~{{ .. }} => {index}u32,@).apply(printer);
                }
            }
        }
    });

    frag!(
        {header}@
        pub fn __type(&self) -> u32~{{
            let mut result = 0u32;@
            let access_result = self.src.access(&mut |src|~{{
                result = match src~{{
                    {variants}
                }};@
                Ok(())
            }});@
            if access_result.is_err()~{{
                unreachable!("enum type cannot be read while its binding method is active");
            }}@
            result
        }}
    )
}

fn compile_field_getter<'db>(
    db: &'db dyn Db,
    variant: EnumVariant<'db>,
    field: ShapeField<'db>,
) -> impl FragmentOnce + 'db {
    let api = Api::get(db);
    let name = field.enum_binding_name(db, variant);
    let getter_name = field.getter_binding_name_on_enum(db, variant);
    let field_type = field.type_(db);
    let result_type = api.result_type(db, compile_boundary_out_type(db, field_type));
    let tethered_value = compile_tethered_struct_get_field_expr(frag!(self.src), name);
    let output = fragment(move |printer| match field_type {
        Type::Struct(_) | Type::Enum(_) => {
            compile_boundary_out_from_tethered_struct_expr(db, tethered_value, field_type)
                .apply(printer);
        }
        Type::Primitive(_) | Type::Str | Type::String => {
            compile_boundary_out_from_source_type_expr(db, tethered_value, field_type)
                .apply(printer);
        }
    });

    frag!(
        {api.field_getter_header(db, name)}@
        pub fn ~{getter_name}(&self)~ -> ~{result_type}~{{
            Ok({output})
        }}
    )
}

fn compile_field_setter<'db>(
    db: &'db dyn Db,
    variant: EnumVariant<'db>,
    field: ShapeField<'db>,
) -> impl FragmentOnce + 'db {
    let api = Api::get(db);
    let name = field.enum_binding_name(db, variant);
    let setter_name = field.setter_binding_name_on_enum(db, variant);
    let param_type_name = compile_boundary_in_type(db, field.type_(db));
    let result_type = api.result_type(db, "()");
    let value = BoundaryInExpr {
        in_expr: "value",
        out_var_name: "value",
        type_: field.type_(db),
        mode: BoundaryMode::Move,
    };
    let set_tethered_field = compile_tethered_struct_set_field_stmt(frag!(self.src), name, "value");

    frag!(
        {api.field_setter_header(db, name)}@
        pub fn ~{setter_name}(&self, value: ~{param_type_name})~ -> ~{result_type}~{{
            let value = ~{compile_boundary_in_to_source_type_expr(db, [value], "value")};@
            {set_tethered_field}
            Ok(())
        }}
    )
}

fn compile_variant_field_access_expr<'db>(
    db: &'db dyn Db,
    enum_: Enum<'db>,
    variant: EnumVariant<'db>,
    field: ShapeField<'db>,
    source: &'static str,
) -> impl FragmentOnce + 'db {
    fragment(move |printer| {
        let api = Api::get(db);
        let reason = frag!(
            {
                doubles((
                    "cannot access inactive enum variant field `",
                    enum_.fqn(db),
                    "::",
                    variant.name(db),
                    "::",
                    field.name(db),
                    "`",
                ))
            }
            .to_string()
        );
        let error = api.error(db, reason);
        let variant_name = variant.name(db);

        match variant.shape(db) {
            Shape::Unit => unreachable!(),
            Shape::Tuple(fields) => {
                let index = fields
                    .iter()
                    .position(|candidate| candidate.as_ref() == Some(&field))
                    .expect("enum field must belong to its variant");
                let pattern = fragment(move |p| {
                    for _ in 0..index {
                        frag!(_,).apply(p);
                    }
                    frag!(field, ..).apply(p);
                });

                frag!(
                    match ~{source}~{{
                        {enum_.fqn(db)}::{variant_name}({pattern}) => Ok(field),@
                        _ => Err({error}),
                    }}
                )
                .apply(printer);
            }
            Shape::Plain { .. } => {
                let field_name = field.name(db);
                frag!(
                    match ~{source}~{{
                        {enum_.fqn(db)}::{variant_name}~{{ {field_name}: field, .. }} => Ok(field),@
                        _ => Err({error}),
                    }}
                )
                .apply(printer);
            }
        }
    })
}

/// Compiles an expression that wraps a tethered enum in its binding struct.
pub fn compile_binding_enum_from_tethered_expr<'db>(
    db: &'db dyn Db,
    enum_id: EnumId<'db>,
    tethered_struct_expr: impl FragmentOnce + 'db,
) -> impl FragmentOnce + 'db {
    let binding_name = enum_id.binding_name(db);
    frag!({binding_name}~{{ src: {tethered_struct_expr} }})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        api::{Api, ApiKind, WasmTarget},
        db::DbImpl,
        fragment::FragmentPrint,
        rustdoc::{
            RustdocId,
            enum_::{EnumLowerError, LoweredEnum, lower_enum as lower_ir_enum},
        },
        test::{assert_snap, test_db},
    };

    fn lower_enum<'db>(
        db: &'db DbImpl,
        id: RustdocId<'db>,
    ) -> Result<Enum<'db>, Box<EnumLowerError>> {
        match lower_ir_enum(db, id)? {
            LoweredEnum::Enum(enum_) => Ok(enum_),
            LoweredEnum::CLikeEnum(_) => panic!("test requires a data-carrying enum"),
        }
    }

    #[test]
    fn compiles_enum_with_tuple_variant() {
        assert_snap!({
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

            Api::set(&mut db, ApiKind::Node);
            let enum_ = lower_enum(&db, db.rustdoc_id("Example")).unwrap();
            let node = compile_binding_enum(&db, enum_).to_string();
            syn::parse_file(&node).expect("generated Node enum bindings must be valid Rust syntax");

            Api::set(
                &mut db,
                ApiKind::Wasm {
                    target: WasmTarget::Web,
                },
            );
            let enum_ = lower_enum(&db, db.rustdoc_id("Example")).unwrap();
            let wasm = compile_binding_enum(&db, enum_).to_string();
            syn::parse_file(&wasm).expect("generated Wasm enum bindings must be valid Rust syntax");

            ("// Node\n", node, "\n\n// Wasm\n", wasm).print()
        });
    }

    #[test]
    fn compiles_enum_with_struct_variant() {
        assert_snap!({
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

            Api::set(&mut db, ApiKind::Node);
            let enum_ = lower_enum(&db, db.rustdoc_id("Example")).unwrap();
            let node = compile_binding_enum(&db, enum_).to_string();
            syn::parse_file(&node).expect("generated Node enum bindings must be valid Rust syntax");

            Api::set(
                &mut db,
                ApiKind::Wasm {
                    target: WasmTarget::Web,
                },
            );
            let enum_ = lower_enum(&db, db.rustdoc_id("Example")).unwrap();
            let wasm = compile_binding_enum(&db, enum_).to_string();
            syn::parse_file(&wasm).expect("generated Wasm enum bindings must be valid Rust syntax");

            ("// Node\n", node, "\n\n// Wasm\n", wasm).print()
        });
    }

    #[test]
    fn compiles_mixed_enum() {
        assert_snap!({
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

            Api::set(&mut db, ApiKind::Node);
            let enum_ = lower_enum(&db, db.rustdoc_id("Example")).unwrap();
            let node = compile_binding_enum(&db, enum_).to_string();
            syn::parse_file(&node).expect("generated Node enum bindings must be valid Rust syntax");

            Api::set(
                &mut db,
                ApiKind::Wasm {
                    target: WasmTarget::Web,
                },
            );
            let enum_ = lower_enum(&db, db.rustdoc_id("Example")).unwrap();
            let wasm = compile_binding_enum(&db, enum_).to_string();
            syn::parse_file(&wasm).expect("generated Wasm enum bindings must be valid Rust syntax");

            ("// Node\n", node, "\n\n// Wasm\n", wasm).print()
        });
    }
}
