mod fn_;
mod mod_;
mod printer;
mod type_;

pub use fn_::*;
pub use mod_::*;
pub use printer::*;
pub use type_::*;

use crate::codegen::{api::Api, db::Db, diag::Diag, rustdoc};

/// Compiles the whole IR into bindings code for the active API.
pub fn compile(db: &dyn Db) -> String {
    let api = Api::get(db);
    let mut out = String::new();
    out.push_str("#![allow(non_camel_case_types, non_snake_case)]\n");
    out.push_str(api.prelude(db));

    for mod_id in rustdoc::all_mod_ids(db) {
        let compiled = compile_mod(db, *mod_id);
        if compiled.trim().is_empty() {
            continue;
        }

        out.push('\n');
        out.push_str(compiled);
    }

    out
}

/// Returns diagnostics accumulated while lowering rustdoc for binding
/// generation.
pub fn get_compile_diags(db: &dyn Db) -> Vec<Diag> {
    let mut diags = Vec::new();

    for mod_id in rustdoc::all_mod_ids(db) {
        rustdoc::lower_mod_fns(db, *mod_id);
        diags.extend(
            rustdoc::lower_mod_fns::accumulated::<Diag>(db, *mod_id)
                .into_iter()
                .cloned(),
        );
    }

    diags
}
