//! Module for compiling all supported items in a Rust module to bindings code.

use crate::codegen::{
    bindings::{fn_::compile_binding_fn, struct_::compile_binding_struct},
    db::Db,
    fragment::{FragmentPrint, spaced},
    rustdoc::{
        RustdocId,
        mod_::{lower_mod_fns, lower_mod_structs},
    },
};

/// Compiles all items inside the module to bindings code.
#[salsa::tracked(returns(ref))]
pub fn compile_mod(db: &dyn Db, mod_id: RustdocId<'_>) -> String {
    let fns = lower_mod_fns(db, mod_id);
    let structs = lower_mod_structs(db, mod_id);

    spaced({
        fns.iter().map(|fn_| compile_binding_fn(db, *fn_)).chain(
            structs
                .iter()
                .map(|struct_| compile_binding_struct(db, *struct_)),
        )
    })
    .print()
}
