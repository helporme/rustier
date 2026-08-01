//! Module for compiling structs into binding structs that can be exposed to
//! JavaScript.

use crate::{
    codegen::{
        api::Api,
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
        fragment::{FragmentOnce, FragmentPrint, fragment, spaced},
        ir::{
            item::{ItemIdExt as _, ItemKind},
            shape::ShapeField,
            struct_::{Struct, StructId},
            type_::{IrType, ItemIdExt as _},
        },
        rustdoc::mod_::{all_mod_ids, lower_mod_enums, lower_mod_structs},
    },
    frag,
};

/// Compiles a Rust struct into its tethered backing struct and public binding
/// wrapper.
#[salsa::tracked(returns(ref))]
pub fn compile_binding_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> String {
    compile_binding_struct_with(db, BindingsCast::Graph, struct_)
}

fn compile_binding_struct_with<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    struct_: Struct<'db>,
) -> String
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    let tethered_fields = struct_.fields(db).iter().map(|field| {
        let name = field.name(db);

        // todo: maybe move the __tethering::AccessResult::Ok() to the tethering.rs
        // itself, and use it here and in enums
        let get_ref_expr =
            move |source: &'static str| frag!(__tethering::AccessResult::Ok(&{source}.{name}));
        let get_mut_expr =
            move |source: &'static str| frag!(__tethering::AccessResult::Ok(&mut ~{source}.{name}));

        field.to_tethered_field(db, get_ref_expr, get_mut_expr)
    });

    let tethering_struct = TetheringStruct {
        source_type: BirSourceType::Item {
            target: struct_.id(db).into(),
        },
        fields: tethered_fields,
    };

    frag!(
        {compile_tethered_struct(db, tethering_struct)}@@
        {compile_wrapper_struct(db, struct_)}@@
        {compile_wrapper_impl(db, cast_compiler, struct_)}
    )
    .print()
}

/// Returns whether compiling any lowered struct or enum requires the tethering
/// prelude.
#[salsa::tracked]
pub fn is_using_tethering(db: &dyn Db) -> bool {
    all_mod_ids(db).iter().any(|mod_id| {
        !lower_mod_structs(db, *mod_id).is_empty() || !lower_mod_enums(db, *mod_id).is_empty()
    })
}

fn compile_wrapper_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);

    fragment(move |p| {
        let source_type = struct_.into_ir_type(db).source_type_owned();
        let tethered_struct_name = source_type.tethering_struct_name(db);

        frag!(
            {api.struct_header(db, struct_)}
            pub struct ~{struct_.extern_name(db)}~{{
                src: std::rc::Rc<{tethered_struct_name}>
            }}
        )
        .apply(p);
    })
}

fn compile_wrapper_impl<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    struct_: Struct<'db>,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    let api = Api::get(db);

    let fields = spaced(struct_.fields(db).iter().copied().map(move |field| {
        let cast_compiler = cast_compiler.clone();

        fragment(move |p| {
            frag!({compile_field_getter(db, cast_compiler.clone(), field)}@).apply(p);

            if !field.is_readonly(db) {
                frag!(@{compile_field_setter(db, cast_compiler, field)}).apply(p);
            }
        })
    }));

    frag!(
        {api.impl_header(db)}@
        impl~{struct_.extern_name(db)}~{{
            {fields}
        }}
    )
}

/// Compiles the public binding getter for a source struct field.
fn compile_field_getter<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    field: ShapeField<'db>,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + 'db,
{
    let api = Api::get(db);
    let getter_name = field.getter_binding_name_on_struct(db);
    let field_type = field.type_(db);
    let result_type = api.result_type(db, field_type.boundary_out().fqn_decl(db));

    let field_value = compile_tethering_struct_get_field_expr(frag!(self.src), field.name(db));

    let output = fragment(move |p| {
        let field_source_type = field_type.source_type_owned();
        if field_source_type.is_tethered(db) {
            cast_compiler
                .compile_cast_expr(
                    db,
                    TetheringStructType {
                        source_type: field_source_type,
                    },
                    field_type.boundary_out(),
                    field_value,
                )
                .apply(p);
        } else {
            cast_compiler
                .compile_cast_expr(
                    db,
                    field_source_type,
                    field_type.boundary_out(),
                    field_value,
                )
                .apply(p);
        }
    });

    frag!(
        {api.field_getter_header(db, field.name(db))}@
        pub fn ~{getter_name}(&self)~ -> ~{result_type}~ {{
            Ok({output})
        }}
    )
}

/// Compiles the public binding setter for a source struct field.
fn compile_field_setter<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    field: ShapeField<'db>,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + 'db,
{
    let api = Api::get(db);

    let setter_name = field.setter_binding_name_on_struct(db);
    let result_type = api.result_type(db, "()");
    let field_boundary_in_type = field.type_(db).boundary_in();
    let field_source_type = field.type_(db).source_type_owned();

    let set_tethered_field =
        compile_tethering_struct_set_field_stmt(frag!(self.src), field.name(db), "value");

    fragment(move |p| {
        let set_tethered_field = cast_compiler.compile_cast_stmt(
            db,
            Cast {
                from: field_boundary_in_type.clone(),
                to: field_source_type,
                in_expr: "value",
                out_var_name: "value",
            },
            set_tethered_field,
        );

        frag!(
            {api.field_setter_header(db, field.name(db))}@
            pub fn ~{setter_name}(&self, value: ~{field_boundary_in_type.fqn_decl(db)})~ -> ~{result_type}~ {{
                {set_tethered_field}
                Ok(())
            }}
        ).apply(p);
    })
}

/// Casts a generated struct boundary value to a tethering struct type.
pub struct CastStructBoundaryTypeToTetheringStructType;

impl<'db> DirectCastRule<'db, BirBoundaryType<'db>, TetheringStructType<'db>>
    for CastStructBoundaryTypeToTetheringStructType
{
    fn predecessor(
        self,
        db: &'db dyn Db,
        to: TetheringStructType<'db>,
    ) -> Option<BirBoundaryType<'db>> {
        match to.source_type {
            BirSourceType::Item { target } if target.kind(db) == ItemKind::Struct => {
                Some(IrType::Struct(StructId::new(db, target).unwrap()).boundary_in())
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
pub struct CastTetheringStructTypeToStructBoundaryType;

impl<'db> DirectCastRule<'db, TetheringStructType<'db>, BirBoundaryType<'db>>
    for CastTetheringStructTypeToStructBoundaryType
{
    fn predecessor(
        self,
        db: &'db dyn Db,
        to: BirBoundaryType<'db>,
    ) -> Option<TetheringStructType<'db>> {
        match to {
            BirBoundaryType::Item { target } if target.kind(db) == ItemKind::Struct => {
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
            unreachable!("struct boundary casts require a generated boundary target");
        };

        frag!({target.extern_name(db)}~{{ src: {in_expr} }})
    }
}
