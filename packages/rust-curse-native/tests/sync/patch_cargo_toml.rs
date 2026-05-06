use std::fs;

use rust_curse::sync::patch_cargo_toml::{
    patch_cargo_toml, write_cargo_toml_file, CargoTomlPatch, Dep, PatchCargoTomlError,
};

/// Apply `patch` twice and assert the second pass produces identical output to
/// the first (idempotency), then return a snapshot-friendly report bundling
/// input, patch, and output so the reviewer can evaluate the test from the
/// snapshot file alone.
fn apply(content: &str, patch: &CargoTomlPatch) -> String {
    let once = patch_cargo_toml(content, patch).unwrap();
    let twice = patch_cargo_toml(&once, patch).unwrap();
    assert_eq!(once, twice, "patch must be idempotent");
    format!(
        "## Input\n\n```toml\n{content}```\n\n## Patch\n\n```\n{patch}```\n\n## Output\n\n```toml\n{once}```\n"
    )
}

const PKG: &str = "[package]\nname = \"x\"\nversion = \"0.1\"\n";

#[test]
fn empty_patch() {
    insta::assert_snapshot!(apply(PKG, &CargoTomlPatch::new()));
}

#[test]
fn lib_path_only() {
    let patch = CargoTomlPatch::new().lib_path("src/lib.rs".into());
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn lib_path_overwrites_existing() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[lib]\npath = \"old.rs\"\n";
    let patch = CargoTomlPatch::new().lib_path("new.rs".into());
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn build_path_only() {
    let patch = CargoTomlPatch::new().build_path("build.rs".into());
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn build_path_overwrites_existing() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\nbuild = \"old.rs\"\n";
    let patch = CargoTomlPatch::new().build_path("new.rs".into());
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn crate_type_into_empty() {
    let patch = CargoTomlPatch::new().crate_type("cdylib".into());
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn crate_type_already_present() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[lib]\ncrate-type = [\"cdylib\"]\n";
    let patch = CargoTomlPatch::new().crate_type("cdylib".into());
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn crate_type_extends() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[lib]\ncrate-type = [\"rlib\"]\n";
    let patch = CargoTomlPatch::new().crate_type("cdylib".into());
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn crate_type_writes_sorted_array() {
    let patch = CargoTomlPatch::new()
        .crate_type("staticlib".into())
        .crate_type("cdylib".into())
        .crate_type("rlib".into());
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn dep_simple() {
    let patch = CargoTomlPatch::new().dep(Dep::new("anyhow".into(), "1".into()));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn dep_with_features() {
    let patch = CargoTomlPatch::new()
        .dep(Dep::new("serde".into(), "1".into()).features(vec!["derive".into(), "rc".into()]));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn dep_with_require_features() {
    let patch = CargoTomlPatch::new()
        .dep(Dep::new("napi".into(), "3".into()).require_features(vec!["node".into()]));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn dep_with_features_and_require_features() {
    let patch = CargoTomlPatch::new().dep(
        Dep::new("napi".into(), "3".into())
            .features(vec!["napi6".into()])
            .require_features(vec!["node".into()]),
    );
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn build_dep_simple() {
    let patch = CargoTomlPatch::new().build_dep(Dep::new("napi-build".into(), "2".into()));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn dep_overwrites_existing_version() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[dependencies]\nserde = \"0.9\"\n";
    let patch = CargoTomlPatch::new().dep(Dep::new("serde".into(), "1".into()));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn existing_features_entry_preserved() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[features]\nnode = [\"dep:other\"]\n";
    let patch = CargoTomlPatch::new()
        .dep(Dep::new("napi".into(), "3".into()).require_features(vec!["node".into()]));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn existing_features_dep_already_listed() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[features]\nnode = [\"dep:napi\"]\n";
    let patch = CargoTomlPatch::new()
        .dep(Dep::new("napi".into(), "3".into()).require_features(vec!["node".into()]));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn full_realistic_patch() {
    let patch = CargoTomlPatch::new()
        .lib_path("src/lib.rs".into())
        .build_path("build.rs".into())
        .crate_type("cdylib".into())
        .crate_type("rlib".into())
        .dep(Dep::new("serde".into(), "1".into()).features(vec!["derive".into()]))
        .dep(Dep::new("napi".into(), "3".into()).require_features(vec!["node".into()]))
        .dep(
            Dep::new("wasm-bindgen".into(), "0.2".into())
                .require_features(vec!["wasm".into(), "node".into()]),
        )
        .build_dep(Dep::new("napi-build".into(), "2".into()));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn empty_patch_is_noop_semantically() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[dependencies]\nanyhow = \"1\"\n[features]\nflag = []\n";
    let baseline: toml_edit::DocumentMut = cargo.parse().unwrap();
    let out = patch_cargo_toml(cargo, &CargoTomlPatch::new()).unwrap();
    assert_eq!(out, baseline.to_string());
}

#[test]
fn dep_with_empty_features_vec_omits_features_key() {
    let patch = CargoTomlPatch::new().dep(Dep::new("a".into(), "1".into()).features(vec![]));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn dep_with_empty_require_features_vec_does_not_optional() {
    let patch =
        CargoTomlPatch::new().dep(Dep::new("a".into(), "1".into()).require_features(vec![]));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn multiple_deps_share_one_require_feature() {
    let patch = CargoTomlPatch::new()
        .dep(Dep::new("b".into(), "1".into()).require_features(vec!["wasm".into()]))
        .dep(Dep::new("a".into(), "1".into()).require_features(vec!["wasm".into()]));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn lib_path_creates_lib_table_when_missing() {
    let patch = CargoTomlPatch::new().lib_path("src/lib.rs".into());
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn build_path_creates_package_table_when_missing() {
    let cargo = "[workspace]\nmembers = []\n";
    let patch = CargoTomlPatch::new().build_path("build.rs".into());
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn preserves_other_top_level_tables() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n\n[workspace]\nmembers = [\"a\"]\n\n[profile.release]\nlto = true\n";
    let patch = CargoTomlPatch::new().dep(Dep::new("anyhow".into(), "1".into()));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn parse_error_returns_error_variant() {
    let bad = "this is not [valid toml";
    match patch_cargo_toml(bad, &CargoTomlPatch::new()) {
        Err(PatchCargoTomlError::Parse(_)) => {}
        other => panic!("expected Parse error, got {:?}", other),
    }
}

#[test]
fn preserves_workspace_inherited_dep_inline_table() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[dependencies]\nnapi = { workspace = true }\n";
    let patch = CargoTomlPatch::new().dep(Dep::new("napi".into(), "3".into()));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn preserves_workspace_inherited_dep_full_table() {
    let cargo =
        "[package]\nname = \"x\"\nversion = \"0.1\"\n[dependencies.napi]\nworkspace = true\n";
    let patch = CargoTomlPatch::new().dep(Dep::new("napi".into(), "3".into()));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn preserves_workspace_inherited_build_dep() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[build-dependencies]\nnapi-build = { workspace = true }\n";
    let patch = CargoTomlPatch::new().build_dep(Dep::new("napi-build".into(), "2".into()));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn workspace_dep_still_gets_feature_injection() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[dependencies]\nnapi = { workspace = true }\n";
    let patch = CargoTomlPatch::new()
        .dep(Dep::new("napi".into(), "3".into()).require_features(vec!["node".into()]));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn with_workspace_dep() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[dependencies]\nnapi = { workspace = true }\nanyhow = \"0.9\"\n";
    let patch = CargoTomlPatch::new()
        .dep(Dep::new("napi".into(), "3".into()))
        .dep(Dep::new("anyhow".into(), "1".into()));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn write_cargo_toml_file_matches_in_memory_variant() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("Cargo.toml");
    let original = "[package]\nname = \"x\"\nversion = \"0.1\"\n";
    fs::write(&p, original).unwrap();
    let patch = CargoTomlPatch::new().dep(Dep::new("anyhow".into(), "1".into()));
    write_cargo_toml_file(&p, &patch).unwrap();
    let on_disk = fs::read_to_string(&p).unwrap();
    let in_memory = patch_cargo_toml(original, &patch).unwrap();
    assert_eq!(
        on_disk, in_memory,
        "file wrapper must delegate to the in-memory patch verbatim",
    );
}

#[test]
fn merge_combines_two_patches() {
    let a = CargoTomlPatch::new()
        .dep(Dep::new("a".into(), "1".into()))
        .crate_type("cdylib".into());
    let b = CargoTomlPatch::new()
        .dep(Dep::new("b".into(), "2".into()))
        .crate_type("rlib".into())
        .lib_path("src/lib.rs".into());
    let merged = a.merge(b);
    assert_eq!(merged.deps.as_ref().unwrap().len(), 2);
    assert_eq!(merged.crate_type.as_ref().unwrap().len(), 2);
    assert_eq!(merged.lib_path.as_deref(), Some("src/lib.rs"));
}
