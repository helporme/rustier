mod bir;
mod cast_graph;
mod class;
mod enum_;
mod extern_fn;
mod extern_free_fn;
mod mod_;
mod primitive;
mod shape;
mod struct_;
mod tethering;

#[cfg(test)]
#[doc(hidden)]
pub mod test;

use crate::codegen::{
    bindings::{
        mod_::compile_mod, struct_::is_using_tethering, tethering::compile_tethering_prelude,
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
