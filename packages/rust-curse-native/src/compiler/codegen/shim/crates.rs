use crate::compiler::{
    codegen::fragment::{FragmentPrint as _, spaced},
    db::Db,
    ir::shim::ShimCrate,
};

#[salsa::tracked]
impl<'db> ShimCrate<'db> {
    /// Renders a self-contained shim code for this crate, including all
    /// referenced (used) members from other crates.
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_self_contained(self, db: &'db dyn Db) -> String {
        let refs = self.refs(db);

        let classes = refs.classes(db);
        let functions = refs.free_functions(db);
        let class_functions = classes.iter().flat_map(|c| c.all_functions(db).iter());

        let members = functions.iter().map(|f| f.as_fn().render(db)).chain(classes.iter().)

        // let members = spaced(self.src(db).refs(db).all(db).iter().map(|ref_| match
        // ref_ {     Reference::Struct(s) => s.shim(db).render(db),
        //     Reference::Enum(e) => e.shim(db).render(db),
        //     Reference::Function(f) => f.
        // }));

        let members = spaced(self.root(db).iter().flat_map(|mod_| {
            mod_.classes(db)
                .iter()
                .map(|class| class.render(db))
                .chain(mod_.fns(db).iter().map(|fn_| fn_.render(db)))
        }));

        members.print()
    }
}

// #[salsa::tracked]
// impl<'db> Shim<'db> {
//     #[salsa::tracked(returns(ref))]
//     pub fn render(self, db: &'db dyn Db) -> String {
//         let platform = Platform::get(db);
//         let shim = frag!(
//             #![allow(unused_mut, dead_code, unused_imports,
// nonstandard_style, unreachable_patterns)]@@
// {platform.prelude()}@@             {compile_tethering_prelude(db)}@@
//             {spaced(self.crates(db).iter().map(|c| c.render(db)))}
//         );

//         shim.print()
//     }
// }
