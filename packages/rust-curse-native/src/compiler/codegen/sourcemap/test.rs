use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::compiler::{
    codegen::sourcemap::mappings::Mapping,
    db::DbImpl,
    inputs::{FfiLayout, Platform},
    ir::{
        decals::DecalMod,
        rust::{Crate, Member},
    },
    test::TEST_CRATE_NAME,
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

pub fn configure_db(db: &mut DbImpl) {
    Platform::set(db, Platform::Node);
    FfiLayout::set(
        db,
        FfiLayout {
            ffi_js_file: std::path::PathBuf::from("ffi.js"),
        },
    );
}

pub fn root_decals<'db>(db: &'db DbImpl) -> DecalMod<'db> {
    Crate::by(db, TEST_CRATE_NAME)
        .expect("test crate should be lowered")
        .root(db)
        .shim(db)
        .decals(db)
}

pub fn decals_by_path<'db>(db: &'db DbImpl, path: &str) -> DecalMod<'db> {
    let mut module = Crate::by(db, TEST_CRATE_NAME)
        .expect("test crate should be lowered")
        .root(db);

    for segment in path.split("::") {
        module = module
            .members(db)
            .iter()
            .find_map(|member| match member {
                Member::Mod(child) if child.name(db) == segment => Some(*child),
                _ => None,
            })
            .expect("test module should be lowered");
    }

    module.shim(db).decals(db)
}

pub fn snapshot_module(db: &DbImpl, module: DecalMod<'_>) -> String {
    let crate_dir = Crate::by(db, TEST_CRATE_NAME)
        .expect("test crate should be lowered")
        .dir(db);
    let generated = numbered_lines(module.render_ts(db));
    let mappings = normalize_mappings(crate_dir, module.render_sourcemap_mappings(db));
    let mut source_map: TestSourceMap = serde_json::from_str(module.render_sourcemap(db)).unwrap();
    source_map.sources = source_map
        .sources
        .iter()
        .map(|source| normalize_source(crate_dir, source))
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
