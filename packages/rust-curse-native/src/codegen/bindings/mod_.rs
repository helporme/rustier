//! Module for compiling all supported items in a Rust module to bindings code.

use crate::codegen::{
    bindings::{
        extern_enum::compile_extern_enum, extern_free_fn::compile_free_fn,
        extern_struct::compile_extern_struct,
    },
    db::Db,
    fragment::{FragmentPrint, spaced},
    ir::Mod,
};

/// Compiles all items inside the module to bindings code.
#[salsa::tracked(returns(ref))]
pub fn compile_mod<'db>(db: &'db dyn Db, mod_: Mod<'db>) -> String {
    let fns = mod_.fns(db);
    let structs = mod_.structs(db);
    let enums = mod_.enums(db);

    spaced(
        fns.map(|fn_| compile_free_fn(db, fn_))
            .chain(structs.map(|struct_| compile_extern_struct(db, struct_)))
            .chain(enums.map(|enum_| compile_extern_enum(db, enum_))),
    )
    .print()
}
