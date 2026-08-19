use crate::{
    compiler::{
        codegen::{
            fragment::{FragmentPrint as _, spaced},
            shim::tethering::compile_tethering_prelude,
        },
        db::Db,
        inputs::Api,
        ir::shim::Shim,
    },
    frag,
};

mod cast;
mod classes;
mod crates;
mod functions;
mod tethering;
mod types;

#[cfg(test)]
mod test;

#[salsa::tracked]
impl<'db> Shim<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db) -> String {
        let api = Api::get(db);
        let shim = frag!(
            #![allow(unused_mut, dead_code, unused_imports, nonstandard_style, unreachable_patterns)]@@
            {api.prelude()}@@
            {compile_tethering_prelude(db)}@@
            {spaced(self.crates(db).iter().map(|c| c.render(db)))}
        );

        shim.print()
    }
}
