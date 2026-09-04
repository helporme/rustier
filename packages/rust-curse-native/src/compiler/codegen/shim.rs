use crate::compiler::{
    codegen::{
        fragment::{FragmentPrint, frag, spaced},
        shim::tethering::compile_tethering_prelude,
    },
    db::Db,
    inputs::Platform,
    ir::{
        rust::usages::UsageGraph,
        shim::{ShimFunction, ShimMember},
    },
};

mod cast;
mod classes;
mod functions;
mod tethering;
mod types;

#[cfg(test)]
mod test;

#[salsa::tracked]
impl<'db> UsageGraph<'db> {
    /// Renders a shim code only for members of [`Self`].
    #[salsa::tracked(returns(ref))]
    pub fn render_shim(self, db: &'db dyn Db) -> String {
        let members = spaced(
            self.members(db)
                .iter()
                .filter_map(|member| match member.shim(db) {
                    ShimMember::Class(shim_class) => Some(shim_class.render(db)),
                    ShimMember::Function(shim_function_free) => {
                        Some(ShimFunction::from(shim_function_free).render(db))
                    }
                    _ => None,
                }),
        );

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
