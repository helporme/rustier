//! Module for compiling structs into a binding structs that can be exposed to
//! the JavaScript.

use crate::{
    codegen::{
        api::Api,
        bindings::{
            boundary::{
                BoundaryInExpr, BoundaryMode, compile_boundary_in_conversion_expr,
                compile_boundary_in_conversion_stmt, compile_boundary_in_type_name,
                compile_boundary_out_conversion_expr, compile_boundary_out_type_name,
            },
            projection::compile_slot_type_name,
        },
        db::Db,
        fragment::{FragmentOnce, FragmentPrint, fragment, spaced},
        ir::{
            IrId,
            struct_::{
                Struct, StructField, struct_binding_name_from_rustdoc_id,
                struct_fqn_from_rustdoc_id, struct_handle_name_from_rustdoc_id,
            },
            type_::Type,
        },
    },
    frag,
};

/// The name of the binding sturct field holding a handle struct.
const HANDLE_FIELD: &str = "handle";

/// The name of the handle struct field holding the source struct accessor.
const SRC_FIELD: &str = "src";

/// Compiles struct to the struct bindings.
#[salsa::tracked(returns(ref))]
pub fn compile_binding_struct(db: &dyn Db, struct_: Struct<'_>) -> String {
    frag!(
        {compile_handle_struct(db, struct_)}@@
        {compile_handle_impl(db, struct_)}@@
        {compile_handle_accessor_trait_impl(db, struct_)}@@
        {compile_wrapper_struct(db, struct_)}@@
        {compile_wrapper_impl(db, struct_)}
    )
    .print()
}

fn compile_wrapper_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);

    frag!(
        {api.struct_header(db, struct_)}
        pub struct ~{struct_.binding_name(db)}~{{
            {HANDLE_FIELD}: std::rc::Rc<{struct_.handle_name(db)}>
        }}
    )
}

fn compile_handle_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> impl FragmentOnce + 'db {
    let cache_fields = fragment(move |p| {
        for field in struct_.fields(db) {
            if let Type::Struct(field_struct_id) = field.type_(db) {
                let field_type_name =
                    struct_handle_name_from_rustdoc_id(db, field_struct_id.rustdoc_id(db));
                let field_name = field.cache_field_name(db);

                frag!(
                    {field_name}: std::cell::RefCell<Option<std::rc::Weak<{field_type_name}>>>,@
                )
                .apply(p);
            }
        }
    });

    frag!(
        struct ~{struct_.handle_name(db)}~{{
            {SRC_FIELD}: {compile_slot_type_name(struct_.fqn(db))},@
            {cache_fields}
        }}
    )
}

fn compile_handle_impl<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> impl FragmentOnce + 'db {
    let struct_handle_name = struct_.handle_name(db);
    let struct_fqn = struct_.fqn(db);

    let cache_fields = fragment(move |p| {
        for field in struct_.fields(db) {
            if matches!(field.type_(db), Type::Struct(_)) {
                frag!(
                    {field.cache_field_name(db)}: std::cell::RefCell::new(None),@
                )
                .apply(p);
            }
        }
    });

    frag!(
        impl ~{struct_handle_name}~{{
            pub fn new_owned(input: ~{struct_fqn})~ -> Self ~{{
                {struct_handle_name}~{{
                    src: __curse::Slot::owned(input),@
                    {cache_fields.clone()}
                }}
            }}@@

            pub fn new_projected(input: std::rc::Rc<dyn __curse::Accessor<{struct_fqn}>>) -> Self~{{
                {struct_handle_name}~{{
                    src: __curse::Slot::projected(input),@
                    {cache_fields}
                }}
            }}
        }}
    )
}

fn compile_handle_accessor_trait_impl<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
) -> impl FragmentOnce + 'db {
    let struct_handle_name = struct_.handle_name(db);
    let struct_fqn = struct_.fqn(db);

    frag!(
        impl __curse::Accessor<{struct_fqn}> for ~{struct_handle_name}
        {{
            fn access(&self, f: &mut dyn FnMut(&{struct_fqn}))~{{
                self.src.access(f);
            }}@

            fn access_mut(&self, f: &mut dyn FnMut(&mut~{struct_fqn}))~{{
                self.src.access_mut(f);
            }}
        }}
    )
}

fn compile_wrapper_impl<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);

    let fields = spaced(struct_.fields(db).iter().copied().map(move |field| {
        fragment(move |p| {
            frag!({compile_field_getter(db, struct_, field)}@).apply(p);

            if !field.is_readonly(db) {
                frag!(@{compile_field_setter(db, struct_, field)}).apply(p);
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

/// Compiles the associated function for getting a field of a struct.
fn compile_field_getter<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
    field: StructField<'db>,
) -> impl FragmentOnce + 'db {
    let api = Api::get(db);

    let getter_name = field.getter_binding_name(db);
    let result_type = api.result_type(db, compile_boundary_out_type_name(db, field.type_(db)));

    let body = fragment(move |p| {
        if let Type::Struct(_) = field.type_(db) {
            compile_struct_field_getter_body(db, field).apply(p);
        } else {
            compile_primitive_field_getter_body(db, struct_, field).apply(p);
        }
    });

    frag!(
        {api.field_getter_header(db, field)}@
        pub fn ~{getter_name}(&self)~ -> ~{result_type}~ {{
            {body}
        }}
    )
}

/// Compiles the body of the getter for a struct field.
fn compile_struct_field_getter_body<'db>(
    db: &'db dyn Db,
    field: StructField<'db>,
) -> impl FragmentOnce + 'db {
    let field_struct_id = match field.type_(db) {
        Type::Struct(id) => id,
        _ => unreachable!(),
    };

    let field_src_struct_fqn = struct_fqn_from_rustdoc_id(db, field_struct_id.rustdoc_id(db));
    let field_handle_struct_name =
        struct_handle_name_from_rustdoc_id(db, field_struct_id.rustdoc_id(db));
    let field_wrapper_struct_name =
        struct_binding_name_from_rustdoc_id(db, field_struct_id.rustdoc_id(db));
    let field_name = field.name(db);
    let field_cache_name = field.cache_field_name(db);

    frag!(
        if let Some(handle)~ = self.{HANDLE_FIELD}.{field_cache_name}.borrow().as_ref().and_then(std::rc::Weak::upgrade)~{{
            return Ok({field_wrapper_struct_name} {{ handle }});@
        }}@@

        let projection: std::rc::Rc<dyn __curse::Accessor<{field_src_struct_fqn}>> = std::rc::Rc::new(__curse::Projection {{
            parent: self.{HANDLE_FIELD}.clone(),
            project: |parent| &parent.{field_name},
            project_mut: |parent| &mut parent.{field_name},
        }});@@

        let handle = std::rc::Rc::new({field_handle_struct_name}::new_projected(projection));@@

        *self.{HANDLE_FIELD}.{field_cache_name}.borrow_mut() = Some(std::rc::Rc::downgrade(&handle));@@

        Ok({field_wrapper_struct_name} {{ handle }})
    )
}

/// Compile body of a getter for a primitive or struct field.
fn compile_primitive_field_getter_body<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
    field: StructField<'db>,
) -> impl FragmentOnce + 'db {
    let get_self_expr = BoundaryInExpr {
        in_expr: &"self",
        out_var_name: "src",
        type_: struct_.as_type(db),
        mode: BoundaryMode::Ref,
    };

    let conversion_expr = compile_boundary_in_conversion_expr(
        db,
        [get_self_expr],
        fragment(move |p| {
            let expr = compile_boundary_out_conversion_expr(
                db,
                frag!(src.{field.name(db)}),
                field.type_(db),
            );

            if field.type_(db) == Type::String {
                // Clone the string to avoid moving it out from the struct.
                // Strings are always cloned when passing the js boundary, so it's not a
                // performance concern.
                frag!({ expr }.clone()).apply(p);
            } else {
                expr.apply(p);
            }
        }),
    );

    frag!(Ok({ conversion_expr }))
}

/// Compiles the associated function for setting a field of a struct.
fn compile_field_setter<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
    field: StructField<'db>,
) -> impl FragmentOnce + 'db {
    let api = Api::get(db);

    let setter_name = field.setter_binding_name(db);
    let param_type_name = compile_boundary_in_type_name(db, field.type_(db));
    let res_type_name = api.result_type(db, "()");

    let body = fragment(move |p| {
        if let Type::Struct(_) = field.type_(db) {
            compile_struct_field_setter_body(db, field).apply(p);
        } else {
            compile_primitive_field_setter_body(db, struct_, field).apply(p);
        }
    });

    frag!(
        {api.field_setter_header(db, field)}@
        pub fn ~{setter_name}(&self, value: ~{param_type_name})~ -> ~{res_type_name}~ {{
            {body}
        }}
    )
}

/// Compiles the body of the setter for a struct field.
fn compile_struct_field_setter_body<'db>(
    db: &'db dyn Db,
    field: StructField<'db>,
) -> impl FragmentOnce + 'db {
    let value_in_expr = BoundaryInExpr {
        in_expr: &"value",
        out_var_name: "value",
        type_: field.type_(db),
        // struct cannot store references, so field setter cannot take a reference, hence we
        // must move the value
        mode: BoundaryMode::Move,
    };

    frag!(
        let value =~{compile_boundary_in_conversion_expr(db, [value_in_expr], "value".to_string())};@@

        let cached_value = self.{HANDLE_FIELD}
            .{field.cache_field_name(db)}
            .borrow_mut()
            .take()
            .and_then(|handle| handle.upgrade());@@

        let mut old_value = None;@
        let mut value = Some(value);@@

        self.{HANDLE_FIELD}.access_mut(&mut |src|~{{
            old_value = Some(std::mem::replace(
                &mut src.{field.name(db)},
                value.take().unwrap(),
            ));
        }});@@

        if let Some(cached_value)~= cached_value~{{
            cached_value.src.replace_with_owned(
                old_value.unwrap(),
            );
        }}@@

        Ok(())
    )
}

/// Compiles the body of the setter for a primitive field.
fn compile_primitive_field_setter_body<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
    field: StructField<'db>,
) -> impl FragmentOnce + 'db {
    let inputs = [
        BoundaryInExpr {
            in_expr: &"self",
            out_var_name: "src",
            type_: struct_.as_type(db),
            mode: BoundaryMode::Mut,
        },
        BoundaryInExpr {
            in_expr: &"value",
            out_var_name: "value",
            type_: field.type_(db),
            // struct cannot store references, so field setter cannot take a reference, hence we
            // must move the value
            mode: BoundaryMode::Move,
        },
    ];

    frag!(
        {compile_boundary_in_conversion_stmt(db, inputs, frag!(src.{field.name(db)} = value;))}
        Ok(())
    )
}

/// Compiles an expression that creates a new instance of the binding
/// struct from the given `input_src_expr` that returns the source value.
pub fn compile_binding_struct_ctor_expr<'db>(
    db: &'db dyn Db,
    id: IrId<'db>,
    input_src_expr: impl FragmentOnce + 'db,
) -> impl FragmentOnce + 'db {
    let struct_handle_name = struct_handle_name_from_rustdoc_id(db, id.rustdoc_id(db));
    let struct_binding_name = struct_binding_name_from_rustdoc_id(db, id.rustdoc_id(db));

    frag!(
        {struct_binding_name}~{{
            handle: std::rc::Rc::new({struct_handle_name}::new_owned({input_src_expr}))
        }}
    )
}

/// Compiles an expression that returns a reference to the accessor of the
/// source struct.
///
/// Takes an expression that returns a reference to the binding struct.
pub fn compile_binding_struct_accessor_expr<'a>(
    struct_expr: impl FragmentOnce + 'a,
) -> impl FragmentOnce + 'a {
    frag!(
        {struct_expr}.{HANDLE_FIELD}.{SRC_FIELD}
    )
}
