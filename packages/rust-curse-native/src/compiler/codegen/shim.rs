use crate::compiler::{
    codegen::{
        fragment::{FragmentPrint, frag, spaced},
        shim::tethering::compile_tethering_prelude,
    },
    db::Db,
    inputs::Platform,
    ir::shim::{Shim, ShimFunction, ShimMember},
};

mod cast;
mod classes;
mod functions;
mod tethering;
mod types;

#[cfg(test)]
mod test;

impl<'db> Shim<'db> {
    pub fn render(self, db: &'db dyn Db) -> String {
        let members = self.usages.members(db);

        let members = spaced(members.iter().filter_map(|member| {
            match member.shim_reachable_from(db, self.usages) {
                ShimMember::Class(shim_class) => Some(shim_class.render(db)),
                ShimMember::Function(shim_function_free) => {
                    Some(ShimFunction::from(shim_function_free).render(db))
                }
                _ => None,
            }
        }));

        let platform = Platform::get(db);
        let shim = frag!(
            #![allow(unused_mut, dead_code, unused_imports, nonstandard_style, unreachable_patterns)]@@
            {platform.prelude()}@@
            {compile_tethering_prelude(db)}@@
            {members}
        );

        shim.print()
    }
}
