use rust_curse::tools::workspace::locate_dependency;

use crate::utils::{fixture, Fixture};

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

fn fixture_with_deps() -> Fixture {
    let main_manifest = package_manifest("workspace-tool-main")
        + "\n[dependencies]\n\
           workspace-tool-plain-dep = { path = \"../plain-dep\" }\n\
           alias_dep = { package = \"workspace-tool-real-dep\", path = \"../real-dep\" }\n\
           \n[workspace]\n";

    fixture(&[
        ("main/Cargo.toml", main_manifest.as_str()),
        ("main/src/lib.rs", ""),
        (
            "plain-dep/Cargo.toml",
            standalone_manifest("workspace-tool-plain-dep").as_str(),
        ),
        ("plain-dep/src/lib.rs", "pub fn plain_dep_marker() {}\n"),
        (
            "real-dep/Cargo.toml",
            standalone_manifest("workspace-tool-real-dep").as_str(),
        ),
        ("real-dep/src/lib.rs", "pub fn real_dep_marker() {}\n"),
    ])
}

#[test]
fn locates_workspace_package_by_package_name() {
    let project = fixture(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"member-a\"]\nresolver = \"2\"\n",
        ),
        (
            "member-a/Cargo.toml",
            package_manifest("workspace-tool-member-a").as_str(),
        ),
        ("member-a/src/lib.rs", "pub fn member_a_marker() {}\n"),
    ]);

    let manifest_path = locate_dependency(
        &project.root_abs_path.join("Cargo.toml"),
        "workspace-tool-member-a".to_string(),
    )
    .expect("locate workspace package");

    assert_eq!(
        manifest_path,
        project
            .root_abs_path
            .join("member-a/Cargo.toml")
            .canonicalize()
            .unwrap()
    );
}

#[test]
fn locates_workspace_package_by_library_target_name() {
    let project = fixture(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"member-a\"]\nresolver = \"2\"\n",
        ),
        (
            "member-a/Cargo.toml",
            package_manifest("workspace-tool-member-a").as_str(),
        ),
        ("member-a/src/lib.rs", "pub fn member_a_marker() {}\n"),
    ]);

    let manifest_path = locate_dependency(
        &project.root_abs_path.join("Cargo.toml"),
        "workspace_tool_member_a".to_string(),
    )
    .expect("locate workspace package");

    assert_eq!(
        manifest_path,
        project
            .root_abs_path
            .join("member-a/Cargo.toml")
            .canonicalize()
            .unwrap()
    );
}

#[test]
fn locates_direct_dependency() {
    let project = fixture_with_deps();

    let manifest_path = locate_dependency(
        &project.root_abs_path.join("main/Cargo.toml"),
        "workspace-tool-plain-dep".to_string(),
    )
    .expect("locate direct dependency");

    assert_eq!(
        manifest_path,
        project
            .root_abs_path
            .join("plain-dep/Cargo.toml")
            .canonicalize()
            .unwrap()
    );
}

#[test]
fn locates_direct_dependency_by_rename() {
    let project = fixture_with_deps();

    let manifest_path = locate_dependency(
        &project.root_abs_path.join("main/Cargo.toml"),
        "alias_dep".to_string(),
    )
    .expect("locate renamed direct dependency");

    assert_eq!(
        manifest_path,
        project
            .root_abs_path
            .join("real-dep/Cargo.toml")
            .canonicalize()
            .unwrap()
    );
}

#[test]
fn fails_for_unknown_package_name() {
    let project = fixture(&[
        (
            "Cargo.toml",
            standalone_manifest("workspace-tool-unknown").as_str(),
        ),
        ("src/lib.rs", ""),
    ]);

    let err = locate_dependency(
        &project.root_abs_path.join("Cargo.toml"),
        "nonexistent".to_string(),
    )
    .expect_err("was not found");

    assert!(err.to_string().contains("was not found"));
}
