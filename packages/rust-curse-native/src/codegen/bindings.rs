pub mod boundary;
pub mod cast;
pub mod fn_;
pub mod mod_;
pub mod printer;
pub mod projection;
pub mod struct_;

use crate::codegen::{
    api::Api,
    bindings::{
        cast::compile_cast_fns,
        mod_::compile_mod,
        projection::{is_projection_preamble_needed, PROJECTION_RUNTIME},
    },
    db::Db,
    fragment::{frag, fragment_fn, spaced, Fragment, FragmentPrint},
    rustdoc::mod_::all_mod_ids,
};

/// Compiles the bindings using active
/// [`crate::codegen::rustdoc::RustdocCrates`] and
/// [`crate::codegen::api::Api`].
#[salsa::tracked]
pub fn compile_bindings(db: &dyn Db) -> String {
    let api = Api::get(db);
    let cast_fns = compile_cast_fns(db);
    let cast_prelude = if cast_fns.is_empty() {
        ""
    } else {
        api.cast_prelude(db).unwrap_or_default()
    };
    let prelude = frag!(
        #![allow(non_camel_case_types, non_snake_case)]@
        {cast_prelude}{api.prelude(db)}
    );

    let modules = fragment_fn(|printer| {
        for (index, module) in all_mod_ids(db)
            .iter()
            .map(|mod_id| compile_mod(db, *mod_id))
            .filter(|compiled| !compiled.trim().is_empty())
            .enumerate()
        {
            if index > 0 {
                "\n".apply(printer);
            }
            module.apply(printer);
        }
    });

    let projection_runtime = PROJECTION_RUNTIME;
    let sections = [
        Some(&prelude as &dyn Fragment),
        is_projection_preamble_needed(db).then_some(&projection_runtime as &dyn Fragment),
        (!cast_fns.is_empty()).then_some(cast_fns as &dyn Fragment),
        Some(&modules as &dyn Fragment),
    ]
    .into_iter()
    .flatten();

    return spaced(sections).print();
}
