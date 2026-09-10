use itertools::Either;

use crate::compiler::{
    codegen::{
        fragment::{FragmentPrint, frag, spaced},
        shim::tethering::compile_tethering_prelude,
    },
    db::Db,
    ir::shim::{Shim, ShimFunction, ShimMember},
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
    Wasm { target: WasmEnv },
    Node,
}

/// Wasm execution environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
#[non_exhaustive]
pub enum WasmEnv {
    Web,
    Node,
}

#[salsa::tracked]
impl<'db> Shim<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db, platform: Platform) -> String {
        let members = spaced(
            self.crates(db)
                .flat_map(|crate_| crate_.all_modules(db))
                .flat_map(|module| module.members(db))
                .flat_map(|member| match member {
                    ShimMember::Class(shim_class) => Either::Left(
                        std::iter::once(shim_class.render(db, platform)).chain(
                            shim_class
                                .all_functions(db)
                                .iter()
                                .map(|function| function.render(db, platform)),
                        ),
                    ),
                    ShimMember::Function(shim_function_free) => Either::Right(
                        Some(ShimFunction::from(shim_function_free).render(db, platform))
                            .into_iter(),
                    ),
                    ShimMember::Module(_) => Either::Right(None.into_iter()),
                }),
        );

        let shim = frag!(
            #![allow(unused_mut, dead_code, unused_imports, nonstandard_style, unreachable_patterns)]@@
            {platform.prelude()}@@
            {compile_tethering_prelude(platform)}@@
            {members}
        );

        shim.print()
    }
}
