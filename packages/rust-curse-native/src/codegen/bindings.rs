mod api;
mod fn_;
mod mod_;
mod printer;
mod type_;

pub use fn_::*;
pub use mod_::*;
pub use printer::*;
pub use type_::*;

pub use api::Api;

use crate::codegen::{db::Db, rustdoc};

/// Compiles the whole IR into a bindings code for given API.
pub fn compile(db: &dyn Db, api: Api) -> String {
    let mut out = String::new();
    out.push_str("#![allow(non_camel_case_types, non_snake_case)]\n");
    out.push_str(api.prelude());

    for mod_id in rustdoc::all_mod_ids(db) {
        let compiled = compile_mod(db, api, *mod_id);
        if compiled.trim().is_empty() {
            continue;
        }

        out.push('\n');
        out.push_str(compiled);
    }

    out
}
