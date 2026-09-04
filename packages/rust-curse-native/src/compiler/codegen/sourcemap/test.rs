use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::compiler::{
    codegen::sourcemap::mappings::Mapping,
    db::RootDatabase,
    inputs::{FilePaths, Platform},
    ir::{
        decals::DecalModule,
        rust::{Member, Rust},
    },
    path::FilePath,
};

#[derive(Deserialize, Serialize)]
struct TestSourceMap {
    version: u8,
    file: String,
    #[serde(rename = "sourceRoot")]
    source_root: String,
    sources: Vec<String>,
    names: Vec<String>,
    mappings: String,
}

pub fn configure_db(db: &mut RootDatabase) {
    Platform::set(db, Platform::Node);
    FilePaths::set(
        db,
        FilePaths {
            ffi_js_file: FilePath::new_virtual("/ffi.js").expect("test FFI path must be valid"),
        },
    );
}

pub fn root_decals<'db>(db: &'db RootDatabase) -> DecalModule<'db> {
    Rust::get(db)
        .crates(db)
        .first()
        .expect("test crate should be lowered")
        .root(db)
        .shim(db)
        .decals(db)
}

pub fn decals_by_path<'db>(db: &'db RootDatabase, path: &str) -> DecalModule<'db> {
    let mut module = Rust::get(db)
        .crates(db)
        .first()
        .expect("test crate should be lowered")
        .root(db);

    for segment in path.split("::") {
        module = module
            .members(db)
            .iter()
            .find_map(|member| match member {
                Member::Module(child) if child.name(db) == segment => Some(*child),
                _ => None,
            })
            .expect("test module should be lowered");
    }

    module.shim(db).decals(db)
}

pub fn snapshot_module(db: &RootDatabase, module: DecalModule<'_>) -> String {
    let crate_dir = Rust::get(db)
        .crates(db)
        .first()
        .expect("test crate should be lowered")
        .dir(db);
    let generated = numbered_lines(module.render_ts(db));
    let mappings = normalize_mappings(crate_dir.as_path(), module.render_sourcemap_mappings(db));
    let mut source_map: TestSourceMap = serde_json::from_str(module.render_sourcemap(db)).unwrap();
    source_map.sources = source_map
        .sources
        .iter()
        .map(|source| normalize_source(crate_dir.as_path(), source))
        .collect();
    let source_map = serde_json::to_string_pretty(&source_map).unwrap();

    format!("TypeScript:\n{generated}\n\nSource map:\n{source_map}\n\nMappings:\n{mappings:#?}")
}

fn normalize_source(crate_dir: &Path, source: &str) -> String {
    Path::new(source)
        .strip_prefix(crate_dir)
        .unwrap_or_else(|_| Path::new(source))
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn normalize_mappings(crate_dir: &Path, mappings: &[Mapping]) -> Vec<Mapping> {
    mappings
        .iter()
        .cloned()
        .map(|mut mapping| {
            mapping.source = normalize_source(crate_dir, &mapping.source);
            mapping
        })
        .collect()
}

fn numbered_lines(source: &str) -> String {
    source
        .lines()
        .enumerate()
        .map(|(line, source)| format!("{line:>3} | {source}"))
        .collect::<Vec<_>>()
        .join("\n")
}
