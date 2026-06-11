use std::fs;

use crate::common::TestProject;

const PKG: &str = "[package]\nname = \"x\"\nversion = \"0.1\"\n";

/// Seed the project's Cargo.toml with `original`, run
/// `Syncer::write_wired_cargo_toml`, and return a snapshot-friendly report
/// bundling the input and the resulting Cargo.toml.
fn apply(original: &str) -> String {
    let project = TestProject::new();
    let cargo_path = project.abs_root_path.join("Cargo.toml");
    fs::write(&cargo_path, original).unwrap();

    let syncer = project.syncer();
    syncer.write_wired_cargo_toml().unwrap();

    let on_disk = fs::read_to_string(&cargo_path).unwrap();
    format!("## Input\n\n```toml\n{original}```\n\n## Output\n\n```toml\n{on_disk}```\n")
}

#[test]
fn fresh_package() {
    insta::assert_snapshot!(apply(PKG));
}

#[test]
fn preserves_existing_lib_table_crate_type() {
    let original = "[package]\nname = \"x\"\nversion = \"0.1\"\n[lib]\ncrate-type = [\"cdylib\"]\n";
    insta::assert_snapshot!(apply(original));
}

#[test]
fn overwrites_existing_lib_path_and_build_path() {
    let original = "[package]\nname = \"x\"\nversion = \"0.1\"\nbuild = \"old-build.rs\"\n[lib]\npath = \"old-lib.rs\"\n";
    insta::assert_snapshot!(apply(original));
}

#[test]
fn preserves_unrelated_top_level_tables() {
    let original = "[package]\nname = \"x\"\nversion = \"0.1\"\n\n[dependencies]\nanyhow = \"1\"\n\n[profile.release]\nlto = true\n";
    insta::assert_snapshot!(apply(original));
}

#[test]
fn idempotent_when_run_twice() {
    let project = TestProject::new();
    let cargo_path = project.abs_root_path.join("Cargo.toml");
    fs::write(&cargo_path, PKG).unwrap();

    let syncer = project.syncer();
    syncer.write_wired_cargo_toml().unwrap();
    let after_first = fs::read_to_string(&cargo_path).unwrap();
    syncer.write_wired_cargo_toml().unwrap();
    let after_second = fs::read_to_string(&cargo_path).unwrap();

    assert_eq!(
        after_first, after_second,
        "write_wired_cargo_toml must be idempotent",
    );
}

#[test]
fn paths_are_relative_to_project_root() {
    let project = TestProject::new();
    let cargo_path = project.abs_root_path.join("Cargo.toml");
    fs::write(&cargo_path, PKG).unwrap();

    project.syncer().write_wired_cargo_toml().unwrap();

    let on_disk = fs::read_to_string(&cargo_path).unwrap();
    let abs_root_str = project.abs_root_path.to_string_lossy();
    assert!(
        !on_disk.contains(abs_root_str.as_ref()),
        "Cargo.toml must not embed the absolute project root path:\n{on_disk}",
    );
    assert!(
        on_disk.contains("path = \".rust-curse/lib.rs\""),
        "expected POSIX-style relative lib path, got:\n{on_disk}",
    );
    assert!(
        on_disk.contains("build = \".rust-curse/build.rs\""),
        "expected POSIX-style relative build path, got:\n{on_disk}",
    );
}
