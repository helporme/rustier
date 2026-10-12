use itertools::Either;

use crate::compiler::{
    codegen::fragment::{FragmentPrint, frag, spaced},
    db::Db,
    ir::interface::{InterClass, InterFunction, InterMember, Interface},
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
impl<'db> Interface<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db, platform: Platform) -> String {
        let alloc_import = self
            .usages(db)
            .types(db)
            .any(|type_| type_.is_from_alloc_crate(db))
            .then_some(frag!(extern crate alloc;@@));

        let render_class = |class: InterClass<'db>| {
            std::iter::once(class.render(db, platform)).chain(
                class
                    .all_functions(db)
                    .iter()
                    .map(move |function| function.render(db, platform)),
            )
        };

        let members = spaced(
            self.crates(db)
                .flat_map(|crate_| crate_.all_modules(db))
                .flat_map(|module| module.members(db))
                .flat_map(|member| match member {
                    InterMember::Class(class) => Either::Left(Either::Left(render_class(class))),
                    InterMember::Template(template) => {
                        Either::Left(Either::Right(template.classes(db).flat_map(render_class)))
                    }
                    InterMember::Function(interface_function_free) => Either::Right(
                        Some(InterFunction::from(interface_function_free).render(db, platform))
                            .into_iter(),
                    ),
                    InterMember::Module(_) => Either::Right(None.into_iter()),
                }),
        );

        let interface = frag!(
            #![allow(unused_mut, dead_code, unused_imports, nonstandard_style, unreachable_patterns)]@@
            {alloc_import}
            {platform.prelude()}@@
            use rustier_rt::tethering as __tethering;@
            use __tethering::{Accessor as _, AccessorExt as _};@@
            {members}
        );

        interface.print()
    }
}
