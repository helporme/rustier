use crate::compiler::{
    codegen::fragment::{FragmentPrint, frag, spaced},
    db::Db,
    ir::{runtime::Runtime, transport::backend::TransportBackend},
};

mod cast;
mod functions;
mod js;
mod tethering;
mod types;

#[cfg(test)]
mod test;

#[salsa::tracked]
impl<'db> Runtime<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render_rust(self, db: &'db dyn Db, transport: TransportBackend) -> String {
        let alloc_import = self
            .facade(db)
            .usages(db)
            .types(db)
            .any(|type_| type_.is_from_alloc_crate(db))
            .then_some(frag!(extern crate alloc;@@));

        let preamble = self.facade(db).transport(db).render_preamble(db, transport);

        let tetherings = self
            .tetherings(db)
            .iter()
            .map(move |tethering| tethering.render(db));

        let functions = self
            .functions(db)
            .iter()
            .map(move |function| function.render_rust(db, transport));

        let members = spaced(tetherings.chain(functions));

        let runtime = frag!(
            #![allow(unused_mut, dead_code, unused_imports, nonstandard_style, unreachable_patterns)]@@
            {alloc_import}
            {preamble}
            use rustier_ext::runtime::tethering as __tethering;@
            use __tethering::{Accessor as _, AccessorExt as _};@@
            {members}
        );

        runtime.print()
    }
}
