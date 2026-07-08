//! Module for compiling structs into a binding structs that can be exposed to
//! the JavaScript.

use crate::codegen::{
    api::Api,
    bindings::{
        compile_binding_fn_param_conv, compile_binding_fn_param_decl, compile_binding_fn_ret_conv,
        compile_binding_fn_ret_decl, Printer,
    },
    db::Db,
    ir::{FnParam, ParamMode, Struct, StructField, StructShape},
};

/// Compiles struct to the struct bindings.
#[salsa::tracked(returns(ref))]
pub fn compile_binidng_struct(db: &dyn Db, struct_: Struct<'_>) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    out.lines(&api.struct_header(db, struct_));
    out.write_fmt(format_args!("pub struct {} ", struct_.bindings_name(db)));
    out.curlies(|out| {
        out.line_fmt(format_args!(
            "orig: std::cell::RefCell<{}>",
            struct_.fqn(db)
        ));
    });
    out.blank();
    out.blank();

    if struct_.shape(db) != &StructShape::Unit {
        out.lines(api.impl_header(db));
        out.write_fmt(format_args!("impl {} ", struct_.bindings_name(db)));
        out.curlies(|out| match struct_.shape(db) {
            StructShape::Unit => unreachable!(),
            StructShape::Tuple(fields) => {
                for field in fields.iter().flatten() {
                    out.lines(compile_field_getter(db, *field));
                    out.blank();
                    out.lines(compile_field_setter(db, *field));
                    out.blank();
                }
            }
            StructShape::Plain { fields, .. } => {
                for field in fields {
                    out.lines(compile_field_getter(db, *field));
                    out.blank();
                    out.lines(compile_field_setter(db, *field));
                    out.blank();
                }
            }
        });
    }

    out.into()
}

/// Compiles the associated function for getting a field of a struct.
#[salsa::tracked(returns(ref))]
fn compile_field_getter<'db>(db: &'db dyn Db, field: StructField<'db>) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    out.lines(&api.field_getter_header(db, field));
    out.write_fmt(format_args!(
        "pub fn {}(&self) -> {} ",
        field.getter_binding_name(db),
        api.result_type(db, compile_binding_fn_ret_decl(field.type_(db)))
    ));

    out.curlies(|out| {
        out.line("let orig = self.orig.borrow();"); // todo: try_borrow
        out.line_fmt(format_args!(
            // We are fine cloning here, since fiels is either:
            //  - a number, cloning is obvious here
            //  - a string, which is impossible to return by reference to js anyway
            "let result = orig.{}.clone();",
            field.name(db)
        ));
        out.write(&compile_binding_fn_ret_conv(db, field.type_(db)));
        out.line("Ok(result)");
    });

    out.into()
}

/// Compiles the associated function for setting a field of a struct.
#[salsa::tracked(returns(ref))]
fn compile_field_setter<'db>(db: &'db dyn Db, field: StructField<'db>) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    // Struct cannot contain references, so all params are moved.
    let param = FnParam::new(db, field.name(db).clone(), field.type_(db), ParamMode::Move);

    out.lines(&api.field_setter_header(db, field));
    out.write_fmt(format_args!(
        "pub fn {}(&self, {}) -> {} ",
        field.setter_binding_name(db),
        compile_binding_fn_param_decl(db, param),
        api.result_type(db, "()")
    ));

    out.curlies(|out| {
        out.line("let mut orig = self.orig.borrow_mut();"); // todo: try_borrow_mut
        out.lines(compile_binding_fn_param_conv(db, param));
        out.line_fmt(format_args!(
            "orig.{} = {};",
            field.name(db),
            param.binding_name(db)
        ));
        out.line("Ok(())");
    });

    out.into()
}
