//! Module for compiling structs into a binding structs that can be exposed to
//! the JavaScript.

use crate::codegen::{
    api::Api,
    bindings::{
        boundary::{
            compile_boundary_in_conversion_expr, compile_boundary_in_conversion_stmt,
            compile_boundary_in_type_name, compile_boundary_out_conversion_expr,
            compile_boundary_out_type_name, BoundaryInExpr, BoundaryMode, BoundaryOutExpr,
        },
        printer::Printer,
        projection::compile_slot_type_name,
    },
    db::Db,
    ir::{
        struct_::{
            struct_binding_name_from_rustdoc_id, struct_fqn_from_rustdoc_id,
            struct_handle_name_from_rustdoc_id, Struct, StructField,
        },
        type_::Type,
        IrId,
    },
};

/// The name of the binding sturct field holding a handle struct.
const HANDLE_FIELD: &str = "handle";

/// The name of the handle struct field holding the source struct accessor.
const SRC_FIELD: &str = "src";

/// Compiles struct to the struct bindings.
#[salsa::tracked(returns(ref))]
pub fn compile_binding_struct(db: &dyn Db, struct_: Struct<'_>) -> String {
    let mut out = Printer::default();

    out.lines(&compile_handle_struct(db, struct_));
    out.blank();
    out.lines(&compile_handle_impl(db, struct_));
    out.blank();
    out.lines(&compile_handle_accessor_trait_impl(db, struct_));
    out.blank();
    out.lines(&compile_wrapper_struct(db, struct_));
    out.blank();
    out.lines(&compile_wrapper_impl(db, struct_));

    out.into()
}

fn compile_wrapper_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    out.lines(&api.struct_header(db, struct_));
    out.write_fmt(format_args!("pub struct {} ", struct_.binding_name(db)));
    out.curlies(|out| {
        out.line_fmt(format_args!(
            "{HANDLE_FIELD}: std::rc::Rc<{}>",
            struct_.handle_name(db)
        ));
    });

    out.blank();
    out.into()
}

fn compile_handle_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> String {
    let mut out = Printer::default();

    out.write_fmt(format_args!("struct {}", struct_.handle_name(db)));
    out.curlies(|out| {
        out.line_fmt(format_args!(
            "{SRC_FIELD}: {},",
            compile_slot_type_name(struct_.fqn(db))
        ));

        for field in struct_.fields(db) {
            if let Type::Struct(field_struct_id) = field.type_(db) {
                out.line_fmt(format_args!(
                    "{}: std::cell::RefCell<Option<std::rc::Weak<{}>>>,",
                    field.cache_field_name(db),
                    struct_handle_name_from_rustdoc_id(db, field_struct_id.rustdoc_id(db))
                ));
            }
        }
    });
    out.blank();

    out.into()
}

fn compile_handle_impl<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> String {
    let mut out = Printer::default();

    let struct_handle_name = struct_.handle_name(db);
    let struct_fqn = struct_.fqn(db);

    out.write_fmt(format_args!("impl {struct_handle_name} "));
    out.curlies(|out| {
        out.line_fmt(format_args!(
            "pub fn new_owned(input: {struct_fqn}) -> Self "
        ));

        out.curlies(|out| {
            out.write_fmt(format_args!("{struct_handle_name} "));
            out.curlies(|out| {
                out.line_fmt(format_args!("src: __curse::Slot::owned(input),"));

                for field in struct_.fields(db) {
                    if matches!(field.type_(db), Type::Struct(_)) {
                        out.line_fmt(format_args!(
                            "{}: std::cell::RefCell::new(None),",
                            field.cache_field_name(db),
                        ));
                    }
                }
            });
        });
        out.blank();

        out.line_fmt(format_args!(
            "pub fn new_projected(input: std::rc::Rc<dyn __curse::Accessor<{struct_fqn}>>) -> Self "
        ));

        out.curlies(|out| {
            out.write_fmt(format_args!("{struct_handle_name} "));
            out.curlies(|out| {
                out.line_fmt(format_args!("src: __curse::Slot::projected(input),"));

                for field in struct_.fields(db) {
                    if matches!(field.type_(db), Type::Struct(_)) {
                        out.line_fmt(format_args!(
                            "{}: std::cell::RefCell::new(None),",
                            field.cache_field_name(db),
                        ));
                    }
                }
            });
        });
    });
    out.blank();

    out.into()
}

fn compile_handle_accessor_trait_impl<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> String {
    let struct_handle_name = struct_.handle_name(db);
    let struct_fqn = struct_.fqn(db);

    indoc::formatdoc! {r#"
        impl __curse::Accessor<{struct_fqn}> for {struct_handle_name}
        {{
            fn access(&self, f: &mut dyn FnMut(&{struct_fqn})) {{
                self.src.access(f);
            }}

            fn access_mut(&self, f: &mut dyn FnMut(&mut {struct_fqn})) {{
                self.src.access_mut(f);
            }}
        }}
    "#}
}

fn compile_wrapper_impl<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    out.lines(api.impl_header(db));
    out.write_fmt(format_args!("impl {} ", struct_.binding_name(db)));
    out.curlies(|out| {
        for field in struct_.fields(db) {
            out.lines(compile_field_getter(db, struct_, *field));
            out.blank();

            if !field.is_readonly(db) {
                out.lines(compile_field_setter(db, struct_, *field));
                out.blank();
            }
        }
    });

    out.into()
}

/// Compiles the associated function for getting a field of a struct.
#[salsa::tracked(returns(ref))]
fn compile_field_getter<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
    field: StructField<'db>,
) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    out.lines(&api.field_getter_header(db, field));
    out.write_fmt(format_args!(
        "pub fn {}(&self) -> {} ",
        field.getter_binding_name(db),
        api.result_type(db, compile_boundary_out_type_name(db, field.type_(db)))
    ));

    out.curlies(|out| {
        if let Type::Struct(_) = field.type_(db) {
            out.lines(&compile_struct_field_getter_body(db, field));
        } else {
            out.lines(&compile_primitive_field_getter_body(db, struct_, field));
        }
    });

    out.into()
}

/// Compiles the body of the getter for a struct field.
fn compile_struct_field_getter_body<'db>(db: &'db dyn Db, field: StructField<'db>) -> String {
    let field_struct_id = match field.type_(db) {
        Type::Struct(id) => id,
        _ => unreachable!(),
    };

    let mut out = Printer::default();

    let field_src_struct_fqn = struct_fqn_from_rustdoc_id(db, field_struct_id.rustdoc_id(db));
    let field_handle_struct_name =
        struct_handle_name_from_rustdoc_id(db, field_struct_id.rustdoc_id(db));
    let field_wrapper_struct_name =
        struct_binding_name_from_rustdoc_id(db, field_struct_id.rustdoc_id(db));
    let field_name = field.name(db);
    let field_cache_name = field.cache_field_name(db);

    out.lines(&indoc::formatdoc! {r#"
        if let Some(handle) = self.{HANDLE_FIELD}.{field_cache_name}.borrow().as_ref().and_then(std::rc::Weak::upgrade) {{
            return Ok({field_wrapper_struct_name} {{ handle }});
        }}

        let projection: std::rc::Rc<dyn __curse::Accessor<{field_src_struct_fqn}>> = std::rc::Rc::new(__curse::Projection {{
            parent: self.{HANDLE_FIELD}.clone(),
            project: |parent| &parent.{field_name},
            project_mut: |parent| &mut parent.{field_name},
        }});

        let handle = std::rc::Rc::new({field_handle_struct_name}::new_projected(projection));

        *self.{HANDLE_FIELD}.{field_cache_name}.borrow_mut() =
            Some(std::rc::Rc::downgrade(&handle));

        return Ok({field_wrapper_struct_name} {{ handle }});
    "#});

    out.into()
}

/// Compile body of a getter for a primitive or struct field.
fn compile_primitive_field_getter_body<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
    field: StructField<'db>,
) -> String {
    let mut out = Printer::default();

    let get_self_expr = BoundaryInExpr {
        in_expr: "self",
        out_var_name: "src",
        type_: struct_.as_type(db),
        mode: BoundaryMode::Ref,
    };

    let conversion_expr = compile_boundary_in_conversion_expr(db, &[get_self_expr], {
        let get_field_expr = BoundaryOutExpr {
            in_expr: &format!("src.{}", field.name(db)),
            type_: field.type_(db),
        };
        compile_boundary_out_conversion_expr(db, &get_field_expr)
    });

    out.write("Ok");
    out.parens(|out| out.lines(&conversion_expr));

    out.into()
}

/// Compiles the associated function for setting a field of a struct.
#[salsa::tracked(returns(ref))]
fn compile_field_setter<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
    field: StructField<'db>,
) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    out.lines(&api.field_setter_header(db, field));

    let setter_name = field.setter_binding_name(db);
    let param_type_name = compile_boundary_in_type_name(db, field.type_(db));
    let res_type_name = api.result_type(db, "()");

    out.write_fmt(format_args!(
        "pub fn {setter_name}(&self, value: {param_type_name}) -> {res_type_name} ",
    ));

    out.curlies(|out| {
        if let Type::Struct(_) = field.type_(db) {
            out.lines(&compile_struct_field_setter_body(db, field));
        } else {
            out.lines(&compile_primitive_field_setter_body(db, struct_, field));
        }
    });

    out.into()
}

/// Compiles the body of the setter for a struct field.
fn compile_struct_field_setter_body<'db>(db: &'db dyn Db, field: StructField<'db>) -> String {
    let mut out = Printer::default();

    let field_name = field.name(db);
    let field_cache_name = field.cache_field_name(db);

    let value_in_expr = BoundaryInExpr {
        in_expr: "value",
        out_var_name: "value",
        type_: field.type_(db),
        // struct cannot store references, so field setter cannot take a reference, hence we
        // must move the value
        mode: BoundaryMode::Move,
    };

    out.lines_fmt(format_args!(
        "let value = {};",
        compile_boundary_in_conversion_expr(db, &[value_in_expr], "value".to_string(),)
    ));

    out.lines(&indoc::formatdoc! {r#"
        let cached_value = self.{HANDLE_FIELD}.{field_cache_name}.borrow_mut().take().and_then(|handle| handle.upgrade());

        let mut old_value = None;
        let mut value = Some(value);

        self.{HANDLE_FIELD}.access_mut(&mut |src| {{
            old_value = Some(std::mem::replace(
                &mut src.{field_name},
                value.take().unwrap(),
            ));
        }});

        if let Some(cached_value) = cached_value {{
            cached_value.src.replace_with_owned(
                old_value.unwrap(),
            );
        }}

        Ok(())
    "#});

    out.into()
}

/// Compiles the body of the setter for a primitive field.
fn compile_primitive_field_setter_body<'db>(
    db: &'db dyn Db,
    struct_: Struct<'db>,
    field: StructField<'db>,
) -> String {
    let mut out = Printer::default();

    let inputs = [
        BoundaryInExpr {
            in_expr: "self",
            out_var_name: "src",
            type_: struct_.as_type(db),
            mode: BoundaryMode::Mut,
        },
        BoundaryInExpr {
            in_expr: "value",
            out_var_name: "value",
            type_: field.type_(db),
            // struct cannot store references, so field setter cannot take a reference, hence we
            // must move the value
            mode: BoundaryMode::Move,
        },
    ];
    out.lines(&compile_boundary_in_conversion_stmt(db, &inputs, {
        format!("src.{} = value;", field.name(db))
    }));

    out.line("Ok(())");

    out.into()
}

/// Compiles an expression that creates a new instance of the binding
/// struct from the given `input_src_expr` that returns the source value.
#[salsa::tracked(returns(ref))]
pub fn compile_binding_struct_ctor_expr<'db>(
    db: &'db dyn Db,
    id: IrId<'db>,
    input_src_expr: String,
) -> String {
    let struct_handle_name = struct_handle_name_from_rustdoc_id(db, id.rustdoc_id(db));
    let struct_binding_name = struct_binding_name_from_rustdoc_id(db, id.rustdoc_id(db));

    format!("{struct_binding_name} {{ handle: std::rc::Rc::new({struct_handle_name}::new_owned({input_src_expr})) }}"
    )
}

/// Compiles an expression that returns a reference to the accessor of the
/// source struct.
///
/// Takes an expression that returns a reference to the binding struct.
pub fn compile_binding_struct_accessor_expr(struct_expr: &str) -> String {
    format!("({struct_expr}).{HANDLE_FIELD}.{SRC_FIELD}")
}
