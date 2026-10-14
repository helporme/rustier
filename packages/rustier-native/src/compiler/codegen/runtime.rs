use crate::compiler::{
    codegen::fragment::{FragmentPrint, frag, spaced},
    db::Db,
    ir::{abi::transport::AbiTransport, runtime::Runtime},
};

mod cast;
mod functions;
pub mod manifest;
mod tethering;
mod types;

#[cfg(test)]
mod test;

#[salsa::tracked]
impl<'db> Runtime<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db, transport: AbiTransport) -> String {
        let alloc_import = self
            .facade(db)
            .usages(db)
            .types(db)
            .any(|type_| type_.is_from_alloc_crate(db))
            .then_some(frag!(extern crate alloc;@@));

        let tetherings = self
            .tetherings(db)
            .iter()
            .map(move |tethering| tethering.render(db));
        let functions = self
            .functions(db)
            .iter()
            .map(move |function| function.render(db, transport));

        let members = spaced(tetherings.chain(functions));

        let runtime = frag!(
            #![allow(unused_mut, dead_code, unused_imports, nonstandard_style, unreachable_patterns)]@@
            {alloc_import}
            {transport.prelude()}@@
            use rustier_rt::tethering as __tethering;@
            use __tethering::{Accessor as _, AccessorExt as _};@@
            {members}
        );

        runtime.print()
    }
}
