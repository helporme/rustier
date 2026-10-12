use crate::compiler::{
    codegen::fragment::{FragmentPrint, frag, spaced},
    db::Db,
    ir::runtime::Runtime,
};

mod cast;
mod classes;
mod functions;
pub mod manifest;
mod platform;
mod tethering;
mod types;

#[cfg(test)]
mod test;

/// Target platform used for FFI code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
#[non_exhaustive]
pub enum Platform {
    Wasm,
    Node,
}

#[salsa::tracked]
impl<'db> Runtime<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db, platform: Platform) -> String {
        let alloc_import = self
            .interface(db)
            .usages(db)
            .types(db)
            .any(|type_| type_.is_from_alloc_crate(db))
            .then_some(frag!(extern crate alloc;@@));

        let classes = self
            .classes(db)
            .iter()
            .map(move |class| class.render(db, platform));
        let functions = self
            .functions(db)
            .iter()
            .map(move |function| function.render(db, platform));

        let members = spaced(classes.chain(functions));

        let runtime = frag!(
            #![allow(unused_mut, dead_code, unused_imports, nonstandard_style, unreachable_patterns)]@@
            {alloc_import}
            {platform.prelude()}@@
            use rustier_rt::tethering as __tethering;@
            use __tethering::{Accessor as _, AccessorExt as _};@@
            {members}
        );

        runtime.print()
    }
}
