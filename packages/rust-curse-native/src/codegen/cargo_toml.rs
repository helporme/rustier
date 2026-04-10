//! Patch or generate Cargo.toml for build targets.

use crate::model::GeneratedFile;
use crate::session::BuildTarget;
use std::path::PathBuf;
use toml_edit::DocumentMut;

/// Optional dependencies injected per build target.
const NODE_DEPS: &[(&str, &str)] = &[("napi", "3"), ("napi-derive", "3")];
const WASM_DEPS: &[(&str, &str)] = &[("wasm-bindgen", "0.2")];

/// Patch an existing Cargo.toml for the build pipeline.
///
/// Sets `[lib].path`, `[package].build`, and adds per-target features +
/// optional dependencies.
pub fn patch_cargo_toml(
    cargo_content: &str,
    lib_rs_rel_path: &str,
    build_rs_rel_path: &str,
    targets: &[BuildTarget],
    cargo_rel_path: PathBuf,
) -> Result<GeneratedFile, toml_edit::TomlError> {
    let mut doc: DocumentMut = cargo_content.parse()?;

    // [lib].path
    let lib = doc["lib"].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
    let lib_table = lib.as_table_mut().expect("[lib] should be a table");
    lib_table.insert("path", toml_edit::value(lib_rs_rel_path));

    // [package].build
    let pkg = doc["package"].as_table_mut().expect("[package] should exist");
    pkg.insert("build", toml_edit::value(build_rs_rel_path));

    // [features] -> per-target features with user-specified names
    let has_features = targets.iter().any(|t| t.feature_name().is_some());
    if has_features {
        let features = doc["features"]
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        let features_table = features.as_table_mut().expect("[features] should be a table");

        for target in targets {
            let (Some(feature_name), optional_deps) = (target.feature_name(), target_deps(target))
            else {
                continue;
            };

            let mut arr = toml_edit::Array::new();
            for (dep_name, _) in optional_deps {
                arr.push(format!("dep:{dep_name}"));
            }
            features_table.insert(feature_name, toml_edit::value(arr));
        }
    }

    // [dependencies] -> optional deps for non-default targets
    for target in targets {
        let optional_deps = target_deps(target);
        if optional_deps.is_empty() {
            continue;
        }

        let deps = doc["dependencies"]
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        let deps_table = deps.as_table_mut().expect("[dependencies] should be a table");

        for (dep_name, version) in optional_deps {
            deps_table.insert(dep_name, inline_dep(version, true));
        }
    }

    // [build-dependencies] -> napi-build for Node targets
    if targets.iter().any(|t| matches!(t, BuildTarget::Node { .. })) {
        let build_deps = doc["build-dependencies"]
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        let build_deps_table = build_deps
            .as_table_mut()
            .expect("[build-dependencies] should be a table");
        build_deps_table.insert("napi-build", inline_dep("2", false));
    }

    Ok(GeneratedFile {
        rel_path: cargo_rel_path,
        content: doc.to_string(),
    })
}

fn target_deps(target: &BuildTarget) -> &'static [(&'static str, &'static str)] {
    match target {
        BuildTarget::Node { .. } => NODE_DEPS,
        BuildTarget::Wasm { .. } => WASM_DEPS,
        BuildTarget::Default { .. } => &[],
    }
}

/// Build an inline table like `{ version = "X", optional = true }`.
fn inline_dep(version: &str, optional: bool) -> toml_edit::Item {
    let mut table = toml_edit::InlineTable::new();
    table.insert("version", version.into());
    if optional {
        table.insert("optional", true.into());
    }
    toml_edit::value(table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SourceOptions;

    fn node_target() -> BuildTarget {
        BuildTarget::Node {
            js_source: SourceOptions { include: vec![], exclude: vec![] },
            feature_name: "target-node".into(),
        }
    }

    fn wasm_target() -> BuildTarget {
        BuildTarget::Wasm {
            js_source: SourceOptions { include: vec![], exclude: vec![] },
            feature_name: "target-wasm".into(),
        }
    }

    fn default_target() -> BuildTarget {
        BuildTarget::Default {
            js_source: SourceOptions { include: vec![], exclude: vec![] },
        }
    }

    #[test]
    fn adds_lib_and_build_paths() {
        let cargo = "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n";
        let file = patch_cargo_toml(
            cargo,
            ".rust-curse/lib.rs",
            ".rust-curse/build.rs",
            &[default_target()],
            PathBuf::from("Cargo.toml"),
        )
        .unwrap();

        let doc: DocumentMut = file.content.parse().unwrap();
        assert_eq!(doc["lib"]["path"].as_str().unwrap(), ".rust-curse/lib.rs");
        assert_eq!(doc["package"]["build"].as_str().unwrap(), ".rust-curse/build.rs");
        assert_eq!(doc["package"]["name"].as_str().unwrap(), "my-crate");
        // No features or deps for default-only
        assert!(doc.get("features").is_none() || doc["features"].as_table().unwrap().is_empty());
    }

    #[test]
    fn adds_node_features_and_deps() {
        let cargo = "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n";
        let file = patch_cargo_toml(
            cargo,
            ".rust-curse/lib.rs",
            ".rust-curse/build.rs",
            &[node_target()],
            PathBuf::from("Cargo.toml"),
        )
        .unwrap();

        let doc: DocumentMut = file.content.parse().unwrap();
        let node_features = doc["features"]["target-node"].as_array().unwrap();
        assert!(node_features.iter().any(|v| v.as_str() == Some("dep:napi")));
        assert!(node_features.iter().any(|v| v.as_str() == Some("dep:napi-derive")));
        assert_eq!(
            doc["dependencies"]["napi"].as_inline_table().unwrap()["version"].as_str().unwrap(),
            "3",
        );
        assert!(
            doc["dependencies"]["napi"].as_inline_table().unwrap()["optional"]
                .as_bool()
                .unwrap(),
        );
        // napi-build as build dependency
        assert_eq!(
            doc["build-dependencies"]["napi-build"]
                .as_inline_table()
                .unwrap()["version"]
                .as_str()
                .unwrap(),
            "2",
        );
    }

    #[test]
    fn adds_wasm_features_and_deps() {
        let cargo = "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n";
        let file = patch_cargo_toml(
            cargo,
            ".rust-curse/lib.rs",
            ".rust-curse/build.rs",
            &[wasm_target()],
            PathBuf::from("Cargo.toml"),
        )
        .unwrap();

        let doc: DocumentMut = file.content.parse().unwrap();
        let wasm_features = doc["features"]["target-wasm"].as_array().unwrap();
        assert!(wasm_features.iter().any(|v| v.as_str() == Some("dep:wasm-bindgen")));
        assert_eq!(
            doc["dependencies"]["wasm-bindgen"]
                .as_inline_table()
                .unwrap()["version"]
                .as_str()
                .unwrap(),
            "0.2",
        );
    }

    #[test]
    fn custom_feature_name() {
        let cargo = "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n";
        let file = patch_cargo_toml(
            cargo,
            ".rust-curse/lib.rs",
            ".rust-curse/build.rs",
            &[BuildTarget::Node {
                js_source: SourceOptions { include: vec![], exclude: vec![] },
                feature_name: "_my-node".into(),
            }],
            PathBuf::from("Cargo.toml"),
        )
        .unwrap();

        let doc: DocumentMut = file.content.parse().unwrap();
        assert!(doc["features"]["_my-node"].as_array().is_some());
        let features = doc["features"].as_table().unwrap();
        assert!(!features.contains_key("target-node"));
    }

    #[test]
    fn overwrites_existing_lib_path() {
        let cargo = concat!(
            "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n\n",
            "[lib]\npath = \"src/lib.rs\"\ncrate-type = [\"cdylib\"]\n",
        );
        let file = patch_cargo_toml(
            cargo,
            ".rust-curse/lib.rs",
            ".rust-curse/build.rs",
            &[default_target()],
            PathBuf::from("Cargo.toml"),
        )
        .unwrap();

        let doc: DocumentMut = file.content.parse().unwrap();
        assert_eq!(doc["lib"]["path"].as_str().unwrap(), ".rust-curse/lib.rs");
        let crate_type = doc["lib"]["crate-type"].as_array().unwrap();
        assert_eq!(crate_type.get(0).unwrap().as_str().unwrap(), "cdylib");
    }

    #[test]
    fn workspace_member_preserves_existing_fields() {
        let cargo = concat!(
            "[package]\n",
            "name = \"my-crate\"\n",
            "version.workspace = true\n",
            "edition.workspace = true\n",
            "\n",
            "[dependencies]\n",
            "serde = { workspace = true }\n",
            "tokio = { workspace = true, features = [\"rt\"] }\n",
            "\n",
            "[features]\n",
            "default = [\"serde\"]\n",
        );
        let file = patch_cargo_toml(
            cargo,
            ".rust-curse/lib.rs",
            ".rust-curse/build.rs",
            &[node_target()],
            PathBuf::from("Cargo.toml"),
        )
        .unwrap();

        let doc: DocumentMut = file.content.parse().unwrap();
        assert!(doc["package"]["version"]["workspace"].as_bool().unwrap());
        assert!(doc["package"]["edition"]["workspace"].as_bool().unwrap());
        assert!(doc["dependencies"]["serde"]["workspace"].as_bool().unwrap());
        assert!(doc["dependencies"]["tokio"]["workspace"].as_bool().unwrap());
        assert!(doc["features"]["default"].as_array().is_some());
        assert!(doc["features"]["target-node"].as_array().is_some());
        assert!(doc["dependencies"]["napi"].as_inline_table().is_some());
        assert_eq!(doc["lib"]["path"].as_str().unwrap(), ".rust-curse/lib.rs");
    }

    #[test]
    fn multiple_targets_add_all_features() {
        let cargo = "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n";
        let file = patch_cargo_toml(
            cargo,
            ".rust-curse/lib.rs",
            ".rust-curse/build.rs",
            &[default_target(), node_target(), wasm_target()],
            PathBuf::from("Cargo.toml"),
        )
        .unwrap();

        let doc: DocumentMut = file.content.parse().unwrap();
        assert!(doc["features"]["target-node"].as_array().is_some());
        assert!(doc["features"]["target-wasm"].as_array().is_some());
        assert!(doc["dependencies"]["napi"].as_inline_table().is_some());
        assert!(doc["dependencies"]["wasm-bindgen"].as_inline_table().is_some());
    }
}
