use crate::codegen::{
    bindings::{compile_fn, compile_struct, Printer},
    db::Db,
    rustdoc::{lower_mod_fns, lower_mod_structs, RustdocId},
};

/// Compiles all items inside the module to bindings code using the active API.
#[salsa::tracked(returns(ref))]
pub fn compile_mod(db: &dyn Db, mod_id: RustdocId<'_>) -> String {
    let mut out = Printer::default();

    for fn_ in lower_mod_fns(db, mod_id) {
        out.lines(compile_fn(db, *fn_));
        out.blank();
    }

    for struct_ in lower_mod_structs(db, mod_id) {
        out.lines(compile_struct(db, *struct_));
        out.blank();
    }

    out.into()
}
