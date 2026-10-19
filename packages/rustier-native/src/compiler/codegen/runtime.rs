use crate::compiler::{
    codegen::{
        abi::render_js_value_adapter,
        fragment::{FragmentPrint, frag, spaced},
    },
    db::Db,
    ir::{abi::transport::AbiTransport, runtime::Runtime},
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
    pub fn render_rust(self, db: &'db dyn Db, transport: AbiTransport) -> String {
        let alloc_import = self
            .facade(db)
            .usages(db)
            .types(db)
            .any(|type_| type_.is_from_alloc_crate(db))
            .then_some(frag!(extern crate alloc;@@));

        let node_api_version = matches!(transport, AbiTransport::Napi).then_some(frag!(
            #[unsafe(no_mangle)]@
            pub extern "C" fn node_api_module_get_api_version_v1()~ -> i32 {{
                10
            }}@@
        ));
        let js_value_adapter = (matches!(transport, AbiTransport::Napi)
            && self
                .functions(db)
                .iter()
                .any(|function| function.abi(db).uses_js_values(db)))
        .then(render_js_value_adapter);

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
            {transport.prelude()}@@
            use rustier_rt::tethering as __tethering;@
            use __tethering::{Accessor as _, AccessorExt as _};@@
            {node_api_version}
            {js_value_adapter}@@
            {members}
        );

        runtime.print()
    }
}
