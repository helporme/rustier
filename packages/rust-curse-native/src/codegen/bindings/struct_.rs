//! Module for compiling structs into binding structs that can be exposed to
//! JavaScript.

use crate::{
    codegen::{
        api::Api,
        bindings::{
            boundary::{
                BoundaryInExpr, BoundaryMode, compile_boundary_in_to_source_type_expr,
                compile_boundary_in_type, compile_boundary_out_from_source_type_expr,
                compile_boundary_out_from_tethered_struct_expr, compile_boundary_out_type,
            },
            tethering::{
                ClonedField, TetheredField, TetheredStruct, TetheredStructField,
                compile_tethered_struct, compile_tethered_struct_get_field_expr,
                compile_tethered_struct_set_field_stmt,
            },
        },
        db::Db,
        fragment::{FragmentOnce, FragmentPrint, fragment, spaced},
        ir::{
            IrId,
            struct_::{
                Struct, StructField, struct_binding_name_from_ir_id,
                struct_tethered_name_from_ir_id,
            },
            type_::Type,
        },
        rustdoc::mod_::{all_mod_ids, lower_mod_structs},
    },
    frag,
};

/// Compiles a Rust struct into its tethered backing struct and public binding
/// wrapper.
#[salsa::tracked(returns(ref))]
pub fn compile_binding_struct(db: &dyn Db, struct_: Struct<'_>) -> String {
    let tethered_fields = struct_.fields(db).iter().map(|field| {
        let name = field.name(db);
        let type_ = field.type_(db);

        let get_ref_expr = move |source: &'static str| frag!({source}.{name});
        let get_mut_expr = move |source: &'static str| frag!({source}.{name});

        match type_ {
            Type::Struct(id) => TetheredField::TetheredStruct(TetheredStructField {
                name,
                get_ref_expr,
                get_mut_expr,
                source_type: type_.rs_name(db),
                tethered_name: struct_tethered_name_from_ir_id(db, id),
                with_setter: !field.is_readonly(db),
            }),
            _ => TetheredField::Cloned(ClonedField {
                name,
                get_ref_expr,
                get_mut_expr,
                source_type: type_.rs_name(db),
            }),
        }
    });

    frag!(
        {compile_tethered_struct(TetheredStruct {
            source_type: struct_.fqn(db),
            name: struct_.tethered_name(db),
            fields: tethered_fields,
        })}@@
        {compile_wrapper_struct(db, struct_)}@@
        {compile_wrapper_impl(db, struct_)}
    )
    .print()
}

/// Returns whether compiling any lowered struct requires the tethering prelude.
#[salsa::tracked]
pub fn is_using_tethering(db: &dyn Db) -> bool {
    all_mod_ids(db)
        .iter()
        .any(|mod_id| !lower_mod_structs(db, *mod_id).is_empty())
}

fn compile_wrapper_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);

    frag!(
        {api.struct_header(db, struct_)}
        pub struct ~{struct_.binding_name(db)}~{{
            src: std::rc::Rc<{struct_.tethered_name(db)}>
        }}
    )
}

fn compile_wrapper_impl<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);

    let fields = spaced(struct_.fields(db).iter().copied().map(move |field| {
        fragment(move |p| {
            frag!({compile_field_getter(db, field)}@).apply(p);

            if !field.is_readonly(db) {
                frag!(@{compile_field_setter(db, field)}).apply(p);
            }
        })
    }));

    frag!(
        {api.impl_header(db)}@
        impl~{struct_.binding_name(db)}~{{
            {fields}
        }}
    )
}

/// Compiles the public binding getter for a source struct field.
fn compile_field_getter<'db>(db: &'db dyn Db, field: StructField<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);
    let getter_name = field.getter_binding_name(db);
    let field_type = field.type_(db);
    let result_type = api.result_type(db, compile_boundary_out_type(db, field_type));

    let tethered_value = compile_tethered_struct_get_field_expr(frag!(self.src), field.name(db));
    let output = fragment(move |p| match field_type {
        Type::Struct(_) => {
            compile_boundary_out_from_tethered_struct_expr(db, tethered_value, field_type).apply(p);
        }
        _ => compile_boundary_out_from_source_type_expr(db, tethered_value, field_type).apply(p),
    });

    frag!(
        {api.field_getter_header(db, field)}@
        pub fn ~{getter_name}(&self)~ -> ~{result_type}~ {{
            Ok({output})
        }}
    )
}

/// Compiles the public binding setter for a source struct field.
fn compile_field_setter<'db>(db: &'db dyn Db, field: StructField<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);
    let setter_name = field.setter_binding_name(db);
    let param_type_name = compile_boundary_in_type(db, field.type_(db));
    let result_type = api.result_type(db, "()");
    let value = BoundaryInExpr {
        in_expr: "value",
        out_var_name: "value",
        type_: field.type_(db),
        // Struct fields cannot store references, so setters always consume a source value.
        mode: BoundaryMode::Move,
    };
    let set_tethered_field =
        compile_tethered_struct_set_field_stmt(frag!(self.src), field.name(db), "value");

    frag!(
        {api.field_setter_header(db, field)}@
        pub fn ~{setter_name}(&self, value: ~{param_type_name})~ -> ~{result_type}~ {{
            let value = ~{compile_boundary_in_to_source_type_expr(db, [value], "value")};@
            {set_tethered_field}
            Ok(())
        }}
    )
}

/// Compiles an expression that retrieves the Rc of the tethered struct from a
/// binding struct.
///
/// ```
/// use rust_curse::codegen::{
///     bindings::struct_::compile_binding_struct_get_tethered_expr, fragment::FragmentPrint,
/// };
///
/// assert_eq!(
///     compile_binding_struct_get_tethered_expr("value").print(),
///     "value.src",
/// );
/// ```
pub fn compile_binding_struct_get_tethered_expr<'a>(
    binding_struct_expr: impl FragmentOnce + 'a,
) -> impl FragmentOnce + 'a {
    frag!({ binding_struct_expr }.src)
}

/// Compiles an expression that wraps a tethered struct in its binding struct.
///
/// The tethered struct expression must be of type `Rc` of the tethered struct.
///
/// ```ignore (requires a lowered struct ID)
/// compile_binding_struct_from_tethered_expr(db, id, "tethered")
/// ```
pub fn compile_binding_struct_from_tethered_expr<'db>(
    db: &'db dyn Db,
    id: IrId<'db>,
    tethered_struct_expr: impl FragmentOnce + 'db,
) -> impl FragmentOnce + 'db {
    let binding_name = struct_binding_name_from_ir_id(db, id);
    frag!({binding_name}~{{ src: {tethered_struct_expr} }})
}
