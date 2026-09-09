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

impl<'db> Shim<'db> {
    pub fn render(self, db: &'db dyn Db, platform: Platform) -> String {
        let members = self.usages.members(db);

        let members = spaced(members.iter().filter_map(|member| {
            match member.shim_reachable_from(db, self.usages) {
                ShimMember::Class(shim_class) => Some(shim_class.render(db, platform)),
                ShimMember::Function(shim_function_free) => {
                    Some(ShimFunction::from(shim_function_free).render(db, platform))
                }
                _ => None,
            }
        }));

        let shim = frag!(
            #![allow(unused_mut, dead_code, unused_imports, nonstandard_style, unreachable_patterns)]@@
            {platform.prelude()}@@
            {compile_tethering_prelude(platform)}@@
            {members}
        );

        shim.print()
    }
}
