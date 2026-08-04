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

use crate::compiler::{
    api::Api,
    bindings::{mod_::compile_mod, tethering::compile_tethering_prelude},
    db::Db,
    fragment::{FragmentOnce, FragmentPrint, frag, fragment, spaced},
    ir::Mod,
    lower::rustdoc_to_ir::mods::lower_mods,
};

/// Compiles bindings from the active lowered modules using the active
/// [`crate::codegen::api::Api`].
#[salsa::tracked]
pub fn compile_bindings(db: &dyn Db) -> String {
    let modules = lower_mods(db);

    fragment(|p| {
        frag!(
            #![allow(unused_mut, dead_code, unused_imports, nonstandard_style, unreachable_patterns)]@@
        )
        .apply(p);

        Api::get(db).prelude(db).apply(p);

        if is_using_tethering(db, modules) {
            frag!(@@).apply(p);
            compile_tethering_prelude(db).apply(p);
        }

        frag!(@@).apply(p);

        spaced(
            modules
                .iter()
                .map(|mod_| compile_mod(db, *mod_))
                .filter(|compiled| !compiled.trim().is_empty()),
        )
        .apply(p);
    })
    .print()
}

/// Returns whether compiling any lowered struct or enum requires the tethering
/// prelude.
fn is_using_tethering<'db>(db: &'db dyn Db, modules: &[Mod<'db>]) -> bool {
    // todo: remove
    modules
        .iter()
        .any(|mod_| mod_.structs(db).next().is_some() || mod_.enums(db).next().is_some())
}
