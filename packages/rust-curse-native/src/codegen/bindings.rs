mod bir;
mod cast_graph;
mod extern_class;
mod extern_enum;
mod extern_fn;
mod extern_free_fn;
mod extern_struct;
mod mod_;
mod primitive;
mod tethering;

#[cfg(test)]
#[doc(hidden)]
pub mod test;

use crate::codegen::{
    bindings::{mod_::compile_mod, tethering::compile_tethering_prelude},
    db::Db,
    fragment::{FragmentOnce, FragmentPrint, frag, fragment, spaced},
    rustdoc::mod_::{all_mod_ids, lower_mod_enums, lower_mod_structs},
};

/// Compiles the bindings using active
/// [`crate::codegen::rustdoc::RustdocCrates`] and
/// [`crate::codegen::api::Api`].
#[salsa::tracked]
pub fn compile_bindings(db: &dyn Db) -> String {
    fragment(|p| {
        if is_using_tethering(db) {
            frag!(@@).apply(p);
            compile_tethering_prelude(db).apply(p);
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

/// Returns whether compiling any lowered struct or enum requires the tethering
/// prelude.
#[salsa::tracked]
fn is_using_tethering(db: &dyn Db) -> bool {
    // todo: remove
    all_mod_ids(db).iter().any(|mod_id| {
        !lower_mod_structs(db, *mod_id).is_empty() || !lower_mod_enums(db, *mod_id).is_empty()
    })
}
