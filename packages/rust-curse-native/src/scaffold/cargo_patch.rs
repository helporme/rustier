//! Patch the user's `Cargo.toml` from the scaffold section catalog.
//!
//! Tracks scaffold-owned keys via `meta.json` so reruns can remove
//! features/deps that no longer correspond to a section, without
//! touching deps the user added themselves.

use std::collections::{BTreeMap, BTreeSet};

use toml_edit::{Array, DocumentMut, InlineTable, Item, Table, Value};

use super::meta::{CargoState, Meta, MetaActivation};
use super::{Activation, Dep, ScaffoldError, Section};

const HEADER_LINE: &str = "# patched by rust-curse with love <3\n";
const TRAILING_TAG: &str = " # do not change";

pub(crate) struct Patched {
    pub content: String,
    pub cargo_state: CargoState,
}

pub(crate) fn apply(
    cargo_content: &str,
    lib_rs_rel_path: &str,
    build_rs_rel_path: Option<&str>,
    sections: &BTreeMap<String, Section>,
    prev_meta: &Meta,
) -> Result<Patched, ScaffoldError> {
    let mut doc: DocumentMut = cargo_content.parse()?;

    let cargo_state = record_originals(&doc, &prev_meta.cargo_toml);

    set_lib_path(&mut doc, lib_rs_rel_path);
    match build_rs_rel_path {
        Some(path) => set_package_build(&mut doc, path),
        None => unset_package_build(&mut doc, &cargo_state.original_package_build),
    }

    update_features(&mut doc, sections, prev_meta);
    update_deps(&mut doc, "dependencies", sections, prev_meta, dep_iter);
    update_deps(
        &mut doc,
        "build-dependencies",
        sections,
        prev_meta,
        build_dep_iter,
    );

    let mut content = doc.to_string();
    if !content.starts_with(HEADER_LINE) {
        content = format!("{HEADER_LINE}{content}");
    }

    Ok(Patched {
        content,
        cargo_state,
    })
}

fn record_originals(doc: &DocumentMut, prev: &CargoState) -> CargoState {
    let mut state = prev.clone();
    if !state.lib_path_recorded {
        state.original_lib_path = doc
            .get("lib")
            .and_then(|i| i.get("path"))
            .and_then(|i| i.as_str())
            .map(String::from);
        state.lib_path_recorded = true;
    }
    if !state.build_recorded {
        state.original_package_build = doc
            .get("package")
            .and_then(|i| i.get("build"))
            .and_then(|i| i.as_str())
            .map(String::from);
        state.build_recorded = true;
    }
    state
}

fn tagged_string_value(s: &str) -> Item {
    let mut item = toml_edit::value(s);
    if let Some(v) = item.as_value_mut() {
        v.decor_mut().set_suffix(TRAILING_TAG);
    }
    item
}

fn tag_inline_table(t: InlineTable) -> Item {
    let mut value: Value = t.into();
    value.decor_mut().set_suffix(TRAILING_TAG);
    Item::Value(value)
}

fn tag_array(arr: Array) -> Item {
    let mut value: Value = arr.into();
    value.decor_mut().set_suffix(TRAILING_TAG);
    Item::Value(value)
}

fn set_lib_path(doc: &mut DocumentMut, lib_rs_rel_path: &str) {
    let lib = doc["lib"].or_insert(Item::Table(Table::new()));
    let table = lib.as_table_mut().expect("[lib] should be a table");
    table.insert("path", tagged_string_value(lib_rs_rel_path));
    let mut crate_type = Array::new();
    crate_type.push("cdylib");
    crate_type.push("rlib");
    table.insert("crate-type", tag_array(crate_type));
}

fn set_package_build(doc: &mut DocumentMut, build_rs_rel_path: &str) {
    let pkg = doc["package"]
        .as_table_mut()
        .expect("[package] should exist");
    pkg.insert("build", tagged_string_value(build_rs_rel_path));
}

fn unset_package_build(doc: &mut DocumentMut, original: &Option<String>) {
    let pkg = doc["package"]
        .as_table_mut()
        .expect("[package] should exist");
    match original {
        Some(orig) => {
            pkg.insert("build", toml_edit::value(orig.as_str()));
        }
        None => {
            pkg.remove("build");
        }
    }
}

fn update_features(doc: &mut DocumentMut, sections: &BTreeMap<String, Section>, prev_meta: &Meta) {
    let prev_features = scaffold_owned_features_prev(prev_meta);
    let now_features = scaffold_owned_features_now(sections);

    if let Some(features_table) = doc.get_mut("features").and_then(Item::as_table_mut) {
        for stale in prev_features.difference(&now_features) {
            features_table.remove(stale);
        }
    }

    if !now_features.is_empty() {
        let features = doc["features"].or_insert(Item::Table(Table::new()));
        let features_table = features
            .as_table_mut()
            .expect("[features] should be a table");
        for section in sections.values() {
            let Some(Activation::CargoFeature(feature_name)) = &section.activation else {
                continue;
            };
            let mut arr = Array::new();
            for dep in &section.deps {
                arr.push(format!("dep:{}", dep.name));
            }
            features_table.insert(feature_name, tag_array(arr));
        }
    }

    remove_if_empty(doc, "features");
}

fn scaffold_owned_features_prev(prev: &Meta) -> BTreeSet<String> {
    prev.sections
        .values()
        .filter_map(|s| match &s.activation {
            Some(MetaActivation::CargoFeature { name }) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

fn scaffold_owned_features_now(sections: &BTreeMap<String, Section>) -> BTreeSet<String> {
    sections
        .values()
        .filter_map(|s| match &s.activation {
            Some(Activation::CargoFeature(name)) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

fn dep_iter(s: &Section) -> &[Dep] {
    &s.deps
}

fn build_dep_iter(s: &Section) -> &[Dep] {
    &s.build_deps
}

fn meta_dep_iter(s: &super::meta::MetaSection) -> &[super::meta::MetaDep] {
    &s.deps
}

fn meta_build_dep_iter(s: &super::meta::MetaSection) -> &[super::meta::MetaDep] {
    &s.build_deps
}

fn update_deps(
    doc: &mut DocumentMut,
    table_key: &str,
    sections: &BTreeMap<String, Section>,
    prev_meta: &Meta,
    iter: fn(&Section) -> &[Dep],
) {
    let meta_iter: fn(&super::meta::MetaSection) -> &[super::meta::MetaDep] =
        if table_key == "dependencies" {
            meta_dep_iter
        } else {
            meta_build_dep_iter
        };

    let prev_owned: BTreeSet<String> = prev_meta
        .sections
        .values()
        .flat_map(|s| meta_iter(s).iter().map(|d| d.name.clone()))
        .collect();
    let now_owned: BTreeSet<String> = sections
        .values()
        .flat_map(|s| iter(s).iter().map(|d| d.name.clone()))
        .collect();

    if let Some(table) = doc.get_mut(table_key).and_then(Item::as_table_mut) {
        for stale in prev_owned.difference(&now_owned) {
            if !is_workspace_dep(table.get(stale)) {
                table.remove(stale);
            }
        }
    }

    let any_now = sections.values().any(|s| !iter(s).is_empty());
    if any_now {
        let table = doc[table_key].or_insert(Item::Table(Table::new()));
        let table = table.as_table_mut().expect("dependencies-like table");

        for section in sections.values() {
            let optional = section.activation.is_some();
            for dep in iter(section) {
                upsert_dep(table, dep, optional);
            }
        }
    }

    remove_if_empty(doc, table_key);
}

fn remove_if_empty(doc: &mut DocumentMut, key: &str) {
    let is_empty = doc
        .get(key)
        .and_then(Item::as_table)
        .map(|t| t.is_empty())
        .unwrap_or(false);
    if is_empty {
        doc.remove(key);
    }
}

fn upsert_dep(table: &mut Table, dep: &Dep, optional: bool) {
    if is_workspace_dep(table.get(&dep.name)) {
        return;
    }
    let mut t = InlineTable::new();
    t.insert("version", dep.version.clone().into());
    if !dep.features.is_empty() {
        let mut arr = Array::new();
        for f in &dep.features {
            arr.push(f.as_str());
        }
        t.insert("features", arr.into());
    }
    if optional {
        t.insert("optional", true.into());
    }
    table.insert(&dep.name, tag_inline_table(t));
}

fn is_workspace_dep(item: Option<&Item>) -> bool {
    let Some(item) = item else { return false };
    if let Some(t) = item.as_inline_table() {
        if t.get("workspace").and_then(|v| v.as_bool()) == Some(true) {
            return true;
        }
    }
    if let Some(t) = item.as_table() {
        if t.get("workspace")
            .and_then(|v| v.as_value())
            .and_then(|v| v.as_bool())
            == Some(true)
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scaffold::{Activation, BuildContribution};

    fn napi_section() -> Section {
        Section {
            activation: Some(Activation::CargoFeature("node".into())),
            content: Some("// n".into()),
            build_content: Some(BuildContribution::default()),
            deps: vec![
                Dep {
                    name: "napi".into(),
                    version: "3".into(),
                    features: vec![],
                },
                Dep {
                    name: "napi-derive".into(),
                    version: "3".into(),
                    features: vec![],
                },
            ],
            build_deps: vec![Dep {
                name: "napi-build".into(),
                version: "2".into(),
                features: vec![],
            }],
            external_files: vec![],
        }
    }

    fn wasm_section() -> Section {
        Section {
            activation: Some(Activation::CargoFeature("wasm".into())),
            content: Some("// w".into()),
            build_content: None,
            deps: vec![Dep {
                name: "wasm-bindgen".into(),
                version: "0.2".into(),
                features: vec![],
            }],
            build_deps: vec![],
            external_files: vec![],
        }
    }

    #[test]
    fn first_run_records_original_paths() {
        let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[lib]\npath = \"src/lib.rs\"\n";
        let prev = Meta::default();
        let patched = apply(
            cargo,
            ".rust-curse/lib.rs",
            Some(".rust-curse/build.rs"),
            &BTreeMap::new(),
            &prev,
        )
        .unwrap();
        assert!(patched.cargo_state.lib_path_recorded);
        assert_eq!(
            patched.cargo_state.original_lib_path,
            Some("src/lib.rs".into())
        );
        assert!(patched.cargo_state.build_recorded);
        assert_eq!(patched.cargo_state.original_package_build, None);
    }

    #[test]
    fn adds_napi_feature_and_deps() {
        let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n";
        let mut sections = BTreeMap::new();
        sections.insert("napi".into(), napi_section());
        let patched = apply(
            cargo,
            ".rust-curse/lib.rs",
            Some(".rust-curse/build.rs"),
            &sections,
            &Meta::default(),
        )
        .unwrap();
        let doc: DocumentMut = patched.content.parse().unwrap();
        let arr = doc["features"]["node"].as_array().unwrap();
        assert!(arr.iter().any(|v| v.as_str() == Some("dep:napi")));
        assert!(arr.iter().any(|v| v.as_str() == Some("dep:napi-derive")));
        assert_eq!(
            doc["dependencies"]["napi"]
                .as_inline_table()
                .unwrap()
                .get("version")
                .unwrap()
                .as_str(),
            Some("3"),
        );
        assert_eq!(
            doc["build-dependencies"]["napi-build"]
                .as_inline_table()
                .unwrap()
                .get("version")
                .unwrap()
                .as_str(),
            Some("2"),
        );
    }

    #[test]
    fn removes_stale_deps_on_section_removal() {
        let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n";
        let mut sections = BTreeMap::new();
        sections.insert("napi".into(), napi_section());
        let first = apply(
            cargo,
            ".rust-curse/lib.rs",
            Some(".rust-curse/build.rs"),
            &sections,
            &Meta::default(),
        )
        .unwrap();

        let prev = Meta::from_state(&sections, first.cargo_state);
        let second = apply(
            &first.content,
            ".rust-curse/lib.rs",
            Some(".rust-curse/build.rs"),
            &BTreeMap::new(),
            &prev,
        )
        .unwrap();
        let doc: DocumentMut = second.content.parse().unwrap();
        assert!(doc
            .get("dependencies")
            .and_then(|i| i.as_table())
            .map(|t| !t.contains_key("napi") && !t.contains_key("napi-derive"))
            .unwrap_or(true));
        assert!(doc
            .get("features")
            .and_then(|i| i.as_table())
            .map(|t| !t.contains_key("node"))
            .unwrap_or(true));
    }

    #[test]
    fn preserves_workspace_inherited_dep() {
        let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[dependencies]\nnapi = { workspace = true }\n";
        let mut sections = BTreeMap::new();
        sections.insert("napi".into(), napi_section());
        let patched = apply(
            cargo,
            ".rust-curse/lib.rs",
            Some(".rust-curse/build.rs"),
            &sections,
            &Meta::default(),
        )
        .unwrap();
        let doc: DocumentMut = patched.content.parse().unwrap();
        let napi = doc["dependencies"]["napi"].as_inline_table().unwrap();
        assert_eq!(napi.get("workspace").and_then(|v| v.as_bool()), Some(true));
        assert!(napi.get("version").is_none());
    }

    #[test]
    fn multiple_sections_compose() {
        let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n";
        let mut sections = BTreeMap::new();
        sections.insert("napi".into(), napi_section());
        sections.insert("wasm".into(), wasm_section());
        let patched = apply(
            cargo,
            ".rust-curse/lib.rs",
            Some(".rust-curse/build.rs"),
            &sections,
            &Meta::default(),
        )
        .unwrap();
        let doc: DocumentMut = patched.content.parse().unwrap();
        assert!(doc["features"]["node"].as_array().is_some());
        assert!(doc["features"]["wasm"].as_array().is_some());
        assert!(doc["dependencies"]["napi"].as_inline_table().is_some());
        assert!(doc["dependencies"]["wasm-bindgen"]
            .as_inline_table()
            .is_some());
    }
}
