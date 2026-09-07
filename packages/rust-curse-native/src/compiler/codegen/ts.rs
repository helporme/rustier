//! Compiles IR into TypeScript declarations.

mod classes;
mod functions;
mod imports;
mod jsdoc;
mod modules;
mod types;

#[cfg(test)]
mod test;

use crate::compiler::{
    codegen::fragment::{FragmentPrint, doubles, frag, spaced},
    db::Db,
    ir::decals::Decals,
};

impl<'db> Decals<'db> {
    /// Renders external declarations as ambient modules, ordered by virtual
    /// path.
    pub fn render_external_modules(self, db: &'db dyn Db) -> String {
        spaced(self.external_modules(db).iter().map(|module| {
            frag!(
                declare module ~{doubles(module.virtual_path(db))} ~{{
                    {module.render_ts(db)}
                }}
            )
        }))
        .print()
    }
}
