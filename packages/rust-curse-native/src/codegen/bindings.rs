mod cast;
mod fn_;
mod mod_;
mod printer;
mod struct_;
mod type_;

pub use cast::*;
pub use fn_::*;
pub use mod_::*;
pub use printer::*;
pub use struct_::*;
pub use type_::*;

use crate::codegen::{api::Api, db::Db, rustdoc};

/// Compiles the whole IR into bindings code for the active API.
#[salsa::tracked]
pub fn compile(db: &dyn Db) -> String {
    let api = Api::get(db);
    let mut out = String::new();

    out.push_str("#![allow(non_camel_case_types, non_snake_case)]\n");

    let cast_fns = compile_cast_fns(db);
    if !cast_fns.is_empty() {
        if let Some(prelude) = api.cast_prelude(db) {
            out.push_str(prelude);
        }
    }
    out.push_str(api.prelude(db));
    if !cast_fns.is_empty() {
        out.push('\n');
        out.push_str(cast_fns);
    }

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
