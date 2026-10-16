//! Generates source map mappings for TypeScript function declarations.

use crate::compiler::{
    codegen::sourcemap::mappings::{Mapping, original_location},
    db::Db,
    ir::facade::FacadeFreeFunction,
};

#[salsa::tracked]
impl<'db> FacadeFreeFunction<'db> {
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
        let (name_line, name_column) = original
            .name
            .unwrap_or((original.line, original.column + "pub fn ".len()));
        let name = self.name(db).to_owned();
        let prefix = if &name == self.decl_name(db) {
            "export function "
        } else {
            "declare function "
        };

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
                generated_column: prefix.len(),
                source: original.source,
                original_line: name_line,
                original_column: name_column,
                name,
            },
        ]
    }
}
