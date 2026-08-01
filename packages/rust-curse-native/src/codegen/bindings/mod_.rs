//! Module for compiling all supported items in a Rust module to bindings code.

use crate::codegen::{
    bindings::{
        extern_enum::compile_extern_enum, extern_free_fn::compile_free_fn,
        extern_struct::compile_extern_struct,
    },
    db::Db,
    fragment::{FragmentPrint, spaced},
    rustdoc::{
        RustdocId,
        mod_::{lower_mod_enums, lower_mod_fns, lower_mod_structs},
    },
};

/// Compiles all items inside the module to bindings code.
#[salsa::tracked(returns(ref))]
pub fn compile_mod(db: &dyn Db, mod_id: RustdocId<'_>) -> String {
    let fns = lower_mod_fns(db, mod_id);
    let structs = lower_mod_structs(db, mod_id);
    let enums = lower_mod_enums(db, mod_id);

    spaced(
        fns.iter()
            .map(|fn_| compile_free_fn(db, *fn_))
            .chain(
                structs
                    .iter()
                    .map(|struct_| compile_extern_struct(db, *struct_)),
            )
            .chain(enums.iter().map(|enum_| compile_extern_enum(db, *enum_))),
    )
    .print()
}
