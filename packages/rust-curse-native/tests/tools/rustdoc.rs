use rust_curse::tools::rustdoc::run;

use crate::utils::{fixture, has_item_named};

fn package_manifest(name: &str) -> String {
    format!(
        "[package]\n\
         name = \"{name}\"\n\
         version = \"0.0.0\"\n\
         edition = \"2021\"\n\
         publish = false\n"
    )
}

fn standalone_manifest(name: &str) -> String {
    package_manifest(name) + "\n[workspace]\n"
}

#[test]
fn documents_manifest_crate() {
    let project = fixture(&[
        (
            "Cargo.toml",
            standalone_manifest("rustdoc-tool-solo").as_str(),
        ),
        ("src/lib.rs", "pub fn solo_marker() {}\n"),
    ]);
    let manifest_path = project.root_abs_path.join("Cargo.toml");

    let crate_ = run(&manifest_path).expect("generate rustdoc JSON");

    assert!(has_item_named(&crate_, "solo_marker"));
    let json_abs_path = project
        .root_abs_path
        .join("target/rustdoc-json/doc/rustdoc_tool_solo.json");
    assert!(json_abs_path.exists());
}

#[test]
fn documents_workspace_member_by_manifest_path() {
    let project = fixture(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"member-a\"]\nresolver = \"2\"\n",
        ),
        (
            "member-a/Cargo.toml",
            package_manifest("rustdoc-tool-member-a").as_str(),
        ),
        ("member-a/src/lib.rs", "pub fn member_a_marker() {}\n"),
    ]);
    let manifest_path = project.root_abs_path.join("member-a/Cargo.toml");

    let crate_ = run(&manifest_path).expect("generate rustdoc JSON");

    assert!(has_item_named(&crate_, "member_a_marker"));
    assert!(project
        .root_abs_path
        .join("target/rustdoc-json/doc/rustdoc_tool_member_a.json")
        .exists());
}

#[test]
fn fails_for_virtual_workspace_manifest() {
    let project = fixture(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"member-b\"]\nresolver = \"2\"\n",
        ),
        (
            "member-b/Cargo.toml",
            package_manifest("rustdoc-tool-member-b").as_str(),
        ),
        ("member-b/src/lib.rs", ""),
    ]);
    let manifest_path = project.root_abs_path.join("Cargo.toml");

    let err = run(&manifest_path).expect_err("virtual workspace manifest resolves");

    assert!(err.to_string().contains("cannot be documented directly"));
}

#[test]
fn fails_for_package_without_library_target() {
    let project = fixture(&[
        (
            "Cargo.toml",
            standalone_manifest("rustdoc-tool-bin-only").as_str(),
        ),
        ("src/main.rs", "fn main() {}\n"),
    ]);
    let manifest_path = project.root_abs_path.join("Cargo.toml");

    let err = run(&manifest_path).expect_err("bin-only package resolves a library target");

    assert!(err.to_string().contains("does not have a library target"));
}
