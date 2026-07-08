pub mod cast;
pub mod fn_;
pub mod mod_;
pub mod printer;
pub mod projection;
pub mod struct_;
pub mod type_;

pub use cast::*;
pub use fn_::*;
pub use mod_::*;
pub use printer::*;
pub use projection::*;
pub use struct_::*;
pub use type_::*;

use crate::codegen::{api::Api, db::Db, rustdoc};

/// Compiles the bindings using active
/// [`crate::codegen::rustdoc::RustdocCrates`] and
/// [`crate::codegen::api::Api`].
#[salsa::tracked]
pub fn compile_bindings(db: &dyn Db) -> String {
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

    if is_projection_preamble_needed(db) {
        out.push('\n');
        out.push_str(PROJECTION_RUNTIME);
    }

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
