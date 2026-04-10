//! Patch or generate Cargo.toml for build targets.

use crate::model::GeneratedFile;
use std::path::PathBuf;
use toml_edit::DocumentMut;

/// Patch an existing Cargo.toml so that `[lib].path` points to the generated lib.rs.
///
/// `lib_rs_rel_path` is the path relative to the Cargo.toml directory
/// (e.g. `.rust-curse/prelude/lib.rs`).
///
/// Returns a `GeneratedFile` whose `rel_path` is `cargo_rel_path` and whose
/// `content` is the patched TOML string.
pub fn patch_cargo_toml(
    cargo_content: &str,
    lib_rs_rel_path: &str,
    cargo_rel_path: PathBuf,
) -> Result<GeneratedFile, toml_edit::TomlError> {
    let mut doc: DocumentMut = cargo_content.parse()?;

    let lib = doc["lib"].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
    let lib_table = lib.as_table_mut().expect("[lib] should be a table");
    lib_table.insert("path", toml_edit::value(lib_rs_rel_path));

    Ok(GeneratedFile {
        rel_path: cargo_rel_path,
        content: doc.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_lib_path() {
        let cargo = "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n";
        let file =
            patch_cargo_toml(cargo, ".rust-curse/prelude/lib.rs", PathBuf::from("Cargo.toml"))
                .unwrap();

        assert_eq!(file.rel_path, PathBuf::from("Cargo.toml"));

        let doc: DocumentMut = file.content.parse().unwrap();
        assert_eq!(
            doc["lib"]["path"].as_str().unwrap(),
            ".rust-curse/prelude/lib.rs"
        );
        // Original fields preserved
        assert_eq!(doc["package"]["name"].as_str().unwrap(), "my-crate");
    }

    #[test]
    fn overwrites_existing_lib_path() {
        let cargo = concat!(
            "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n\n",
            "[lib]\npath = \"src/lib.rs\"\ncrate-type = [\"cdylib\"]\n",
        );
        let file =
            patch_cargo_toml(cargo, ".rust-curse/prelude/lib.rs", PathBuf::from("Cargo.toml"))
                .unwrap();

        let doc: DocumentMut = file.content.parse().unwrap();
        assert_eq!(
            doc["lib"]["path"].as_str().unwrap(),
            ".rust-curse/prelude/lib.rs"
        );
        // Other [lib] fields preserved
        let crate_type = doc["lib"]["crate-type"].as_array().unwrap();
        assert_eq!(crate_type.get(0).unwrap().as_str().unwrap(), "cdylib");
    }
}
