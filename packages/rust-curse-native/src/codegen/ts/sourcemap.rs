//! Compiles rustdoc into TypeScript declaration source maps.

mod mappings;

use crate::codegen::{
    db::Db,
    ir::fn_::Fn,
    rustdoc::{
        RustdocId,
        mod_::{lower_mod, lower_mod_fns},
    },
};
pub use mappings::Mapping;
use mappings::{compile_mappings, jsdoc_line_count};
use serde::Serialize;
use std::fmt;

/// A Source Map v3 document for generated TypeScript declarations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, salsa::Update)]
pub struct SourceMap {
    version: u8,
    file: String,
    #[serde(rename = "sourceRoot")]
    source_root: String,
    sources: Vec<String>,
    names: Vec<String>,
    mappings: String,
}

impl fmt::Display for SourceMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        f.write_str(&json)
    }
}

/// Compiles a rustdoc module id into a TypeScript declaration source map.
#[salsa::tracked(returns(ref))]
pub fn compile_mod<'db>(db: &'db dyn Db, mod_id: RustdocId<'db>) -> SourceMap {
    let item = mod_id.item(db);
    let file = item
        .span
        .as_ref()
        .and_then(|span| span.filename.file_name())
        .map(|file_name| format!("{}.d.ts", file_name.to_string_lossy()))
        .unwrap_or_default();
    let compiled_mappings = compile_mappings(compile_mod_mappings(db, mod_id));

    SourceMap {
        version: 3,
        file,
        source_root: String::new(),
        sources: compiled_mappings.sources,
        names: compiled_mappings.names,
        mappings: compiled_mappings.encoded,
    }
}

/// Compiles a rustdoc module id into TypeScript declaration source map
/// mappings.
#[salsa::tracked(returns(ref))]
fn compile_mod_mappings<'db>(db: &'db dyn Db, mod_id: RustdocId<'db>) -> Vec<Mapping> {
    let mod_ = lower_mod(db, mod_id);
    let fns = lower_mod_fns(db, mod_id);
    let mut mappings = Vec::new();
    let mut generated_line = 0;
    let mut has_output = false;

    if let Some(docs) = mod_.docs(db) {
        generated_line += jsdoc_line_count(docs);
        has_output = true;
    }

    for fn_ in fns {
        if has_output {
            generated_line += 1;
        }

        if let Some(docs) = fn_.docs(db) {
            generated_line += jsdoc_line_count(docs);
        }

        mappings.extend(
            compile_fn_mappings(db, *fn_, generated_line)
                .iter()
                .cloned(),
        );
        generated_line += 1;
        has_output = true;
    }

    mappings
}

/// Compiles an IR function into TypeScript declaration source map mappings.
#[salsa::tracked(returns(ref))]
fn compile_fn_mappings<'db>(db: &'db dyn Db, fn_: Fn<'db>, generated_line: usize) -> Vec<Mapping> {
    let rustdoc_id = fn_.id(db).rustdoc_id(db);
    let item = rustdoc_id.item(db);
    let Some(span) = &item.span else {
        return Vec::new();
    };

    let crate_ = rustdoc_id.crate_(db);
    let source_path = if span.filename.is_absolute() {
        span.filename.clone()
    } else {
        crate_.dir.join(&span.filename)
    };
    let source = source_path
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let name = fn_.name(db).to_string();
    let original_line = span.begin.0.saturating_sub(1);
    let original_column = span.begin.1.saturating_sub(1);

    vec![
        Mapping {
            generated_line,
            generated_column: 0,
            source: source.clone(),
            original_line,
            original_column,
            name: name.clone(),
        },
        Mapping {
            generated_line,
            generated_column: "export function ".len(),
            source,
            original_line,
            original_column: original_column + "pub fn ".len(),
            name,
        },
    ]
}
