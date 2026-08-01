//! Module for compiling enums into binding structs that can be exposed to
//! JavaScript.

use crate::{
    codegen::{
        api::{Api, ApiKind},
        bindings::{
            bir::{BirBoundaryType, BirSourceType},
            cast_graph::{BindingsCast, BindingsCastCompiler, Cast},
            tethering::{
                TetheringStruct, TetheringStructType, compile_tethered_struct,
                compile_tethering_struct_get_field_expr, compile_tethering_struct_set_field_stmt,
            },
        },
        cast::DirectCastRule,
        db::Db,
        fragment::{FragmentOnce, FragmentPrint, doubles, fragment, spaced},
        ir::{
            enum_::{Enum, EnumId, EnumVariant},
            item::{ItemIdExt as _, ItemKind},
            shape::{Shape, ShapeField},
            type_::{IrType, ItemIdExt},
        },
    },
    frag,
};

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
    compile_binding_enum_with(db, BindingsCast::Graph, enum_)
}

fn compile_binding_enum_with<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    enum_: Enum<'db>,
) -> String
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    let tethered_fields = enum_.variants(db).iter().copied().flat_map(move |variant| {
        variant.fields(db).map(move |field| {
            let get_expr = move |source: &'static str| {
                compile_variant_field_access_expr(db, enum_, variant, field, source)
            };

            field.to_tethered_field(db, get_expr, get_expr)
        })
    });

    let tethering_struct = TetheringStruct {
        source_type: BirSourceType::Item {
            target: enum_.id(db).into(),
        },
        fields: tethered_fields,
    };

    frag!(
        {compile_tethered_struct(db, tethering_struct)}@@
        {compile_wrapper_struct(db, enum_)}@@
        {compile_wrapper_impl(db, cast_compiler, enum_)}
    )
    .print()
}

fn compile_wrapper_struct<'db>(db: &'db dyn Db, enum_: Enum<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);
    let extern_name = enum_.extern_name(db);

    fragment(move |p| {
        let header = api.header_renamed(db, enum_.extern_name(db));
        let tethering_type = enum_.into_ir_type(db).source_type_owned();
        let tethering_type_name = tethering_type.tethering_struct_name(db);

        frag!(
            {header}
            pub struct ~{extern_name}~{{
                src: std::rc::Rc<{tethering_type_name}>
            }}
        )
        .apply(p);
    })
}

fn compile_wrapper_impl<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    enum_: Enum<'db>,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    let api = Api::get(db);
    let fields = spaced(enum_.variants(db).iter().copied().flat_map(move |variant| {
        let cast_compiler = cast_compiler.clone();

        variant.fields(db).map(move |field| {
            let cast_compiler = cast_compiler.clone();

            fragment(move |printer| {
                frag!({compile_field_getter(db, cast_compiler.clone(), variant, field)}@)
                    .apply(printer);

                if !field.is_readonly(db) {
                    frag!(@{compile_field_setter(db, cast_compiler, variant, field)})
                        .apply(printer);
                }
            })
        })
    }));

    frag!(
        {api.impl_header(db)}@
        impl~{enum_.extern_name(db)}~{{
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

fn compile_field_getter<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    variant: EnumVariant<'db>,
    field: ShapeField<'db>,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + 'db,
{
    let api = Api::get(db);
    let name = field.enum_binding_name(db, variant);
    let getter_name = field.getter_binding_name_on_enum(db, variant);
    let field_type = field.type_(db);

    let result_type = api.result_type(db, field_type.boundary_out().fqn_decl(db));

    let field_value = compile_tethering_struct_get_field_expr(frag!(self.src), name);
    let output = fragment(move |p| {
        let source_type = field_type.source_type_owned();
        if source_type.is_tethered(db) {
            cast_compiler
                .compile_cast_expr(
                    db,
                    TetheringStructType { source_type },
                    field_type.boundary_out(),
                    field_value,
                )
                .apply(p)
        } else {
            cast_compiler
                .compile_cast_expr(db, source_type, field_type.boundary_out(), field_value)
                .apply(p)
        }
    });

    frag!(
        {api.field_getter_header(db, name)}@
        pub fn ~{getter_name}(&self)~ -> ~{result_type}~{{
            Ok({output})
        }}
    )
}

fn compile_field_setter<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    variant: EnumVariant<'db>,
    field: ShapeField<'db>,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + 'db,
{
    let api = Api::get(db);
    let name = field.enum_binding_name(db, variant);
    let setter_name = field.setter_binding_name_on_enum(db, variant);
    let result_type = api.result_type(db, "()");

    let boundary_in_type = field.type_(db).boundary_in();
    let source_type = field.type_(db).source_type_owned();

    let set_tethered_field =
        compile_tethering_struct_set_field_stmt(frag!(self.src), name, "value");

    let set_tethered_field = cast_compiler.compile_cast_stmt(
        db,
        Cast {
            from: boundary_in_type.clone(),
            to: source_type,
            in_expr: "value",
            out_var_name: "value",
        },
        set_tethered_field,
    );

    frag!(
        {api.field_setter_header(db, name)}@
        pub fn ~{setter_name}(&self, value: ~{boundary_in_type.fqn_decl(db)})~ -> ~{result_type}~{{
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

/// Casts a generated struct boundary value to a tethering struct type.
pub struct CastEnumBoundaryTypeToTetheringStructType;

impl<'db> DirectCastRule<'db, BirBoundaryType<'db>, TetheringStructType<'db>>
    for CastEnumBoundaryTypeToTetheringStructType
{
    fn predecessor(
        self,
        db: &'db dyn Db,
        to: TetheringStructType<'db>,
    ) -> Option<BirBoundaryType<'db>> {
        match to.source_type {
            BirSourceType::Item { target } if target.kind(db) == ItemKind::Enum => {
                Some(IrType::Enum(EnumId::new(db, target).unwrap()).boundary_in())
            }
            _ => None,
        }
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: BirBoundaryType<'db>,
        _to: TetheringStructType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        frag!({ in_expr }.src)
    }
}

/// Casts a generated struct boundary value to a tethering struct type.
pub struct CastTetheringStructTypeToEnumBoundaryType;

impl<'db> DirectCastRule<'db, TetheringStructType<'db>, BirBoundaryType<'db>>
    for CastTetheringStructTypeToEnumBoundaryType
{
    fn predecessor(
        self,
        db: &'db dyn Db,
        to: BirBoundaryType<'db>,
    ) -> Option<TetheringStructType<'db>> {
        match to {
            BirBoundaryType::Item { target } if target.kind(db) == ItemKind::Enum => {
                Some(TetheringStructType {
                    source_type: BirSourceType::Item { target },
                })
            }
            _ => None,
        }
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: TetheringStructType<'db>,
        to: BirBoundaryType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let BirBoundaryType::Item { target } = to else {
            unreachable!("enum boundary casts require a generated boundary target");
        };

        frag!({target.extern_name(db)}~{{ src: {in_expr} }})
    }
}
