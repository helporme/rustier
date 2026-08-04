use std::path::Path;

use crate::compiler::{db::DbImpl, rustdoc::RustdocId, ts::sourcemap::Mapping};

pub fn normalize_source(crate_dir: &Path, source: &str) -> String {
    Path::new(source)
        .strip_prefix(crate_dir)
        .unwrap_or_else(|_| Path::new(source))
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

pub fn normalize_mappings(crate_dir: &Path, mappings: &[Mapping]) -> Vec<Mapping> {
    mappings
        .iter()
        .cloned()
        .map(|mut mapping| {
            mapping.source = normalize_source(crate_dir, &mapping.source);
            mapping
        })
        .collect()
}

pub fn numbered_lines(source: &str) -> String {
    source
        .lines()
        .enumerate()
        .map(|(line, source)| format!("{line:>3} | {source}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn snapshot_mappings(
    db: &DbImpl,
    rustdoc_id: RustdocId<'_>,
    generated: &str,
    mappings: &[Mapping],
) -> String {
    let crate_dir = &rustdoc_id.crate_(db).dir;
    let generated = numbered_lines(generated);
    let mappings = normalize_mappings(crate_dir, mappings);

    format!("TypeScript:\n{generated}\n\nMappings:\n{mappings:#?}")
}
