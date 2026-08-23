//! Generates source map mappings for TypeScript function declarations.

use crate::compiler::{
    codegen::sourcemap::mappings::{Mapping, original_location},
    db::Db,
    ir::decals::DecalFunction,
};

#[salsa::tracked]
impl<'db> DecalFunction<'db> {
    /// Renders mappings for this function at its declaration line.
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_sourcemap_mappings(
        self,
        db: &'db dyn Db,
        generated_line: usize,
    ) -> Vec<Mapping> {
        let Some(span) = self.span(db) else {
            return Vec::new();
        };
        let original = original_location(db, span);
        let name = self.name(db).to_owned();

        vec![
            Mapping {
                generated_line,
                generated_column: 0,
                source: original.source.clone(),
                original_line: original.line,
                original_column: original.column,
                name: name.clone(),
            },
            Mapping {
                generated_line,
                generated_column: "export function ".len(),
                source: original.source,
                original_line: original.line,
                original_column: original.column + "pub fn ".len(),
                name,
            },
        ]
    }
}
