use std::fs;

use rust_curse::sync::patch_cargo_toml::{
    patch_cargo_toml, patch_cargo_toml_file, CargoTomlPatch, Dep, PatchCargoTomlError,
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
    let patch = CargoTomlPatch::new().lib_path("src/lib.rs");
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn lib_path_overwrites_existing() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[lib]\npath = \"old.rs\"\n";
    let patch = CargoTomlPatch::new().lib_path("new.rs");
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn build_path_only() {
    let patch = CargoTomlPatch::new().build_path("build.rs");
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn build_path_overwrites_existing() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\nbuild = \"old.rs\"\n";
    let patch = CargoTomlPatch::new().build_path("new.rs");
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn crate_type_into_empty() {
    let patch = CargoTomlPatch::new().crate_type("cdylib");
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn crate_type_already_present() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[lib]\ncrate-type = [\"cdylib\"]\n";
    let patch = CargoTomlPatch::new().crate_type("cdylib");
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn crate_type_extends() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[lib]\ncrate-type = [\"rlib\"]\n";
    let patch = CargoTomlPatch::new().crate_type("cdylib");
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn crate_type_rlib_only_is_omitted() {
    let patch = CargoTomlPatch::new().crate_type("rlib");
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn crate_type_existing_rlib_only_is_dropped_when_patched_with_rlib() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[lib]\ncrate-type = [\"rlib\"]\n";
    let patch = CargoTomlPatch::new().crate_type("rlib");
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn crate_type_writes_sorted_array() {
    let patch = CargoTomlPatch::new()
        .crate_type("staticlib")
        .crate_type("cdylib")
        .crate_type("rlib");
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn dep_simple() {
    let patch = CargoTomlPatch::new().dep(Dep::new("anyhow", "1"));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn dep_with_features() {
    let patch = CargoTomlPatch::new().dep(Dep::new("serde", "1").feature("derive").feature("rc"));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn dep_with_require_features() {
    let patch = CargoTomlPatch::new().dep(Dep::new("napi", "3").requires_feature("node"));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn dep_with_features_and_require_features() {
    let patch = CargoTomlPatch::new().dep(
        Dep::new("napi", "3")
            .feature("napi6")
            .requires_feature("node"),
    );
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn build_dep_simple() {
    let patch = CargoTomlPatch::new().build_dep(Dep::new("napi-build", "2"));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn dep_overwrites_existing_version() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[dependencies]\nserde = \"0.9\"\n";
    let patch = CargoTomlPatch::new().dep(Dep::new("serde", "1"));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn existing_features_entry_preserved() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[features]\nnode = [\"dep:other\"]\n";
    let patch = CargoTomlPatch::new().dep(Dep::new("napi", "3").requires_feature("node"));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn existing_features_dep_already_listed() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[features]\nnode = [\"dep:napi\"]\n";
    let patch = CargoTomlPatch::new().dep(Dep::new("napi", "3").requires_feature("node"));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn full_realistic_patch() {
    let patch = CargoTomlPatch::new()
        .lib_path("src/lib.rs")
        .build_path("build.rs")
        .crate_type("cdylib")
        .crate_type("rlib")
        .dep(Dep::new("serde", "1").feature("derive"))
        .dep(Dep::new("napi", "3").requires_feature("node"))
        .dep(
            Dep::new("wasm-bindgen", "0.2")
                .requires_feature("wasm")
                .requires_feature("node"),
        )
        .build_dep(Dep::new("napi-build", "2"));
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
    let patch = CargoTomlPatch::new().dep(Dep::new("a", "1"));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn dep_with_empty_require_features_vec_does_not_optional() {
    let patch = CargoTomlPatch::new().dep(Dep::new("a", "1"));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn multiple_deps_share_one_require_feature() {
    let patch = CargoTomlPatch::new()
        .dep(Dep::new("b", "1").requires_feature("wasm"))
        .dep(Dep::new("a", "1").requires_feature("wasm"));
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn lib_path_creates_lib_table_when_missing() {
    let patch = CargoTomlPatch::new().lib_path("src/lib.rs");
    insta::assert_snapshot!(apply(PKG, &patch));
}

#[test]
fn build_path_creates_package_table_when_missing() {
    let cargo = "[workspace]\nmembers = []\n";
    let patch = CargoTomlPatch::new().build_path("build.rs");
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn preserves_other_top_level_tables() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n\n[workspace]\nmembers = [\"a\"]\n\n[profile.release]\nlto = true\n";
    let patch = CargoTomlPatch::new().dep(Dep::new("anyhow", "1"));
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
    let cargo =
        "[package]\nname = \"x\"\nversion = \"0.1\"\n[dependencies]\nnapi = { workspace = true }\n";
    let patch = CargoTomlPatch::new().dep(Dep::new("napi", "3"));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn preserves_workspace_inherited_dep_full_table() {
    let cargo =
        "[package]\nname = \"x\"\nversion = \"0.1\"\n[dependencies.napi]\nworkspace = true\n";
    let patch = CargoTomlPatch::new().dep(Dep::new("napi", "3"));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn preserves_workspace_inherited_build_dep() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[build-dependencies]\nnapi-build = { workspace = true }\n";
    let patch = CargoTomlPatch::new().build_dep(Dep::new("napi-build", "2"));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn workspace_dep_still_gets_feature_injection() {
    let cargo =
        "[package]\nname = \"x\"\nversion = \"0.1\"\n[dependencies]\nnapi = { workspace = true }\n";
    let patch = CargoTomlPatch::new().dep(Dep::new("napi", "3").requires_feature("node"));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn with_workspace_dep() {
    let cargo = "[package]\nname = \"x\"\nversion = \"0.1\"\n[dependencies]\nnapi = { workspace = true }\nanyhow = \"0.9\"\n";
    let patch = CargoTomlPatch::new()
        .dep(Dep::new("napi", "3"))
        .dep(Dep::new("anyhow", "1"));
    insta::assert_snapshot!(apply(cargo, &patch));
}

#[test]
fn write_cargo_toml_file_matches_in_memory_variant() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("Cargo.toml");
    let original = "[package]\nname = \"x\"\nversion = \"0.1\"\n";
    fs::write(&p, original).unwrap();
    let patch = CargoTomlPatch::new().dep(Dep::new("anyhow", "1"));
    patch_cargo_toml_file(&p, &patch).unwrap();
    let on_disk = fs::read_to_string(&p).unwrap();
    let in_memory = patch_cargo_toml(original, &patch).unwrap();
    assert_eq!(
        on_disk, in_memory,
        "file wrapper must delegate to the in-memory patch verbatim",
    );
}
