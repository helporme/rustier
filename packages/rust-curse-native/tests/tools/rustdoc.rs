use rust_curse::tools::rustdoc::{run, RunOpts};

use crate::utils::{fixture, has_item_named, Fixture};

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
    let main_manifest = package_manifest("rustdoc-tool-main")
        + "\n[dependencies]\n\
           rustdoc-tool-plain-dep = { path = \"../plain-dep\" }\n\
           alias_dep = { package = \"rustdoc-tool-real-dep\", path = \"../real-dep\" }\n\
           \n[workspace]\n";

    fixture(&[
        ("main/Cargo.toml", main_manifest.as_str()),
        ("main/src/lib.rs", ""),
        (
            "plain-dep/Cargo.toml",
            standalone_manifest("rustdoc-tool-plain-dep").as_str(),
        ),
        ("plain-dep/src/lib.rs", "pub fn plain_dep_marker() {}\n"),
        (
            "real-dep/Cargo.toml",
            standalone_manifest("rustdoc-tool-real-dep").as_str(),
        ),
        ("real-dep/src/lib.rs", "pub fn real_dep_marker() {}\n"),
    ])
}

#[test]
fn documents_root_crate() {
    let project = fixture(&[
        (
            "Cargo.toml",
            standalone_manifest("rustdoc-tool-solo").as_str(),
        ),
        ("src/lib.rs", "pub fn solo_marker() {}\n"),
    ]);

    let crate_ = run(RunOpts {
        manifest_path: project.root_abs_path.join("Cargo.toml"),
        crate_name: None,
    })
    .expect("generate rustdoc JSON");

    assert!(has_item_named(&crate_, "solo_marker"));
    let json_abs_path = project
        .root_abs_path
        .join("target/rustdoc-json/doc/rustdoc_tool_solo.json");
    assert!(json_abs_path.exists());
}

#[test]
fn documents_root_crate_by_giving_crate_name() {
    let name = "rustdoc-tool-solo".to_string();
    let project = fixture(&[
        ("Cargo.toml", standalone_manifest(&name).as_str()),
        ("src/lib.rs", "pub fn solo_marker() {}\n"),
    ]);

    let crate_ = run(RunOpts {
        manifest_path: project.root_abs_path.join("Cargo.toml"),
        crate_name: Some(name),
    })
    .expect("generate rustdoc JSON");

    assert!(has_item_named(&crate_, "solo_marker"));
    let json_abs_path = project
        .root_abs_path
        .join("target/rustdoc-json/doc/rustdoc_tool_solo.json");
    assert!(json_abs_path.exists());
}

#[test]
fn documents_workspace_crate() {
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

    for crate_name in ["rustdoc-tool-member-a", "rustdoc_tool_member_a"] {
        let crate_ = run(RunOpts {
            manifest_path: project.root_abs_path.join("Cargo.toml"),
            crate_name: Some(crate_name.to_string()),
        })
        .expect("generate rustdoc JSON");

        assert!(has_item_named(&crate_, "member_a_marker"));
    }

    assert!(project
        .root_abs_path
        .join("target/rustdoc-json/doc/rustdoc_tool_member_a.json")
        .exists());
}

#[test]
fn documents_direct_dependency() {
    let project = fixture_with_deps();

    let crate_ = run(RunOpts {
        manifest_path: project.root_abs_path.join("main/Cargo.toml"),
        crate_name: Some("rustdoc-tool-plain-dep".to_string()),
    })
    .expect("generate rustdoc JSON");

    assert!(has_item_named(&crate_, "plain_dep_marker"));
}

#[test]
fn documents_direct_renamed_dependency() {
    let project = fixture_with_deps();

    let crate_ = run(RunOpts {
        manifest_path: project.root_abs_path.join("main/Cargo.toml"),
        crate_name: Some("alias_dep".to_string()),
    })
    .expect("generate rustdoc JSON");

    assert!(has_item_named(&crate_, "real_dep_marker"));
}

#[test]
fn fails_for_unknown_crate_name() {
    let project = fixture(&[
        (
            "Cargo.toml",
            standalone_manifest("rustdoc-tool-unknown").as_str(),
        ),
        ("src/lib.rs", ""),
    ]);

    let err = run(RunOpts {
        manifest_path: project.root_abs_path.join("Cargo.toml"),
        crate_name: Some("nonexistent".to_string()),
    })
    .expect_err("unknown crate name resolves");

    assert!(err.to_string().contains("was not found"));
}

#[test]
fn fails_for_virtual_workspace_without_crate_name() {
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

    let err = run(RunOpts {
        manifest_path: project.root_abs_path.join("Cargo.toml"),
        crate_name: None,
    })
    .expect_err("virtual workspace resolves without crate name");

    assert!(err.to_string().contains("virtual workspaces require"));
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

    let err = run(RunOpts {
        manifest_path: project.root_abs_path.join("Cargo.toml"),
        crate_name: None,
    })
    .expect_err("bin-only package resolves a library target");

    assert!(err.to_string().contains("does not have a library target"));
}
