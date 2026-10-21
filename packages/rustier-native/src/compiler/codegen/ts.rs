//! Compiles IR into TypeScript declarations.

mod class_templates;
mod classes;
mod functions;
mod imports;
mod jsdoc;
mod modules;
mod traits;
mod types;

#[cfg(test)]
mod test;

use crate::compiler::{
    codegen::fragment::{FragmentPrint, doubles, frag, spaced},
    db::Db,
    ir::{facade::Facade, runtime::RuntimeModule},
};

impl<'db> Facade<'db> {
    /// Renders external declarations as ambient modules.
    pub fn render_ambient_ts_mods(self, db: &'db dyn Db) -> String {
        spaced(self.external_modules(db).iter().map(|module| {
            // An export declaration disables implicit exports in ambient
            // modules, keeping synthetic enum brand symbols private.
            frag!(
                declare module ~{doubles(module.virtual_path(db))} ~{{
                    {module.render_ts_in(db, DeclarationScope::AmbientModule)}@@
                    export {};
                }}
            )
        }))
        .print()
    }
}

impl RuntimeModule {
    pub fn render_ts_mod(self) -> &'static str {
        include_str!("runtime/rustier-root.d.ts")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
enum DeclarationScope {
    Module,
    AmbientModule,
}

impl DeclarationScope {
    /// Nested `declare` modifiers are illegal inside an ambient module.
    fn declare_prefix(self) -> &'static str {
        match self {
            Self::Module => "declare ",
            Self::AmbientModule => "",
        }
    }
}
