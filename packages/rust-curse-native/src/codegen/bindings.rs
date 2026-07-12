pub mod boundary;
pub mod cast;
pub mod fn_;
pub mod mod_;
pub mod projection;
pub mod struct_;

use crate::codegen::{
    api::Api,
    bindings::{
        cast::compile_cast_fns,
        mod_::compile_mod,
        projection::{PROJECTION_RUNTIME, is_projection_preamble_needed},
    },
    db::Db,
    fragment::{FragmentOnce, FragmentPrint, frag, fragment, spaced},
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

    fragment(|p| {
        frag!(
            #![allow(non_camel_case_types, non_snake_case)]@
            {cast_prelude}{api.prelude(db)}
        )
        .apply(p);

        if is_projection_preamble_needed(db) {
            frag!(@@).apply(p);
            PROJECTION_RUNTIME.apply(p);
        }

        if !cast_fns.is_empty() {
            frag!(@@).apply(p);
            cast_fns.apply(p);
        }

        frag!(@@).apply(p);

        spaced(
            all_mod_ids(db)
                .iter()
                .map(|mod_id| compile_mod(db, *mod_id))
                .filter(|compiled| !compiled.trim().is_empty()),
        )
        .apply(p);
    })
    .print()
}
