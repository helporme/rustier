use crate::codegen::{
    api::Api,
    bindings::{
        compile_param_conv, compile_param_decl, compile_ret_conv, compile_ret_decl, Printer,
    },
    db::Db,
    ir::{FnParam, ParamMode, Struct, StructField, StructShape},
};

/// Compiles [`ir::Struct`] to struct bindings using the active API.
#[salsa::tracked(returns(ref))]
pub fn compile_struct(db: &dyn Db, struct_: Struct<'_>) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    out.lines(&api.struct_header(db, struct_));
    out.write_fmt(format_args!("pub struct {} ", struct_.cursed_name(db)));
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
        out.write_fmt(format_args!("impl {} ", struct_.cursed_name(db)));
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

#[salsa::tracked(returns(ref))]
pub fn compile_field_getter<'db>(db: &'db dyn Db, field: StructField<'db>) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    out.lines(&api.field_getter_header(db, field));
    out.write_fmt(format_args!(
        "pub fn {}(&self) -> {} ",
        field.getter_name(db),
        api.result_type(db, compile_ret_decl(field.type_(db)))
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
        out.write(&compile_ret_conv(db, field.type_(db)));
        out.line("Ok(result)");
    });

    out.into()
}

#[salsa::tracked(returns(ref))]
pub fn compile_field_setter<'db>(db: &'db dyn Db, field: StructField<'db>) -> String {
    let api = Api::get(db);
    let mut out = Printer::default();

    // Struct cannot contain references, so all params are moved.
    let param = FnParam::new(db, field.name(db).clone(), field.type_(db), ParamMode::Move);

    out.lines(&api.field_setter_header(db, field));
    out.write_fmt(format_args!(
        "pub fn {}(&self, {}) -> {} ",
        field.setter_name(db),
        compile_param_decl(db, param),
        api.result_type(db, "()")
    ));

    out.curlies(|out| {
        out.line("let mut orig = self.orig.borrow_mut();"); // todo: try_borrow_mut
        out.lines(compile_param_conv(db, param));
        out.line_fmt(format_args!(
            "orig.{} = {};",
            field.name(db),
            param.mangled_name(db)
        ));
        out.line("Ok(())");
    });

    out.into()
}
