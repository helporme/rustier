use crate::codegen::{
    bindings::{compile_fn, Api, Printer},
    db::Db,
    rustdoc,
};

/// Compiles all items inside the module to a bindings code using given API.
#[salsa::tracked(returns(ref))]
pub fn compile_mod(db: &dyn Db, api: Api, mod_id: rustdoc::Id<'_>) -> String {
    let fns = rustdoc::lower_mod_fns(db, mod_id);

    let mut p = Printer::default();

    for fn_ in fns {
        p.write(compile_fn(db, api, *fn_));
        p.blank();
    }

    p.into()
}
