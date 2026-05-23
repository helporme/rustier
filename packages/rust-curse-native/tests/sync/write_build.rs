use std::fs;

use rust_curse::sync::write_build::{
    env_flag_build_content, render_build, write_build, BuildBranch, BuildSwitch,
};

/// Render `switch` twice and assert idempotency, then return a
/// snapshot-friendly markdown report bundling the input switch and the
/// rendered `build.rs`.
fn apply(switch: &BuildSwitch) -> String {
    let once = render_build(switch);
    let twice = render_build(switch);
    assert_eq!(once, twice, "render must be idempotent");
    let body = once.as_deref().unwrap_or("(none)\n");
    format!("## Switch\n\n```\n{switch}```\n\n## build.rs\n\n```rust\n{body}```\n")
}

#[test]
fn empty_switch() {
    insta::assert_snapshot!(apply(&BuildSwitch::new()));
}

#[test]
fn single_branch_no_reqs() {
    let mut switch = BuildSwitch::new();
    switch.set_branch(
        "default",
        BuildBranch::new("println!(\"cargo:rerun-if-changed=src\");\n"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_branch_with_cargo_feature() {
    let mut switch = BuildSwitch::new();
    switch.set_branch(
        "napi",
        BuildBranch::new("napi_build::setup();\n").requires_cargo_feature("node"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_branch_with_multiple_cargo_features() {
    let mut switch = BuildSwitch::new();
    switch.set_branch(
        "combo",
        BuildBranch::new("// combo\n")
            .requires_cargo_feature("a")
            .requires_cargo_feature("b"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_branch_with_use_and_call() {
    let mut switch = BuildSwitch::new();
    switch.set_branch(
        "napi",
        BuildBranch::new("use napi_build;\nnapi_build::setup();\n").requires_cargo_feature("node"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn multiple_branches_each_with_use_and_call() {
    let mut switch = BuildSwitch::new();
    switch
        .set_branch(
            "napi",
            BuildBranch::new("use napi_build;\nnapi_build::setup();\n")
                .requires_cargo_feature("node"),
        )
        .set_branch(
            "wasm",
            BuildBranch::new("use wasm_build;\nwasm_build::setup();\n")
                .requires_cargo_feature("wasm"),
        );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn multiple_branches_sorted() {
    let mut switch = BuildSwitch::new();
    switch
        .set_branch("zeta", BuildBranch::new("// zeta\n"))
        .set_branch("alpha", BuildBranch::new("// alpha\n"))
        .set_branch("middle", BuildBranch::new("// middle\n"));
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn multiline_body_indents_each_line() {
    let mut switch = BuildSwitch::new();
    switch.set_branch(
        "multi",
        BuildBranch::new("let x = 1;\nlet y = 2;\nprintln!(\"{}\", x + y);\n"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn branch_using_env_flag_build_content() {
    let mut switch = BuildSwitch::new();
    switch.set_branch(
        "rustdoc",
        BuildBranch::new(env_flag_build_content("RUST_CURSE_RUSTDOC")),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn env_flag_build_content_uppercase_lowercase() {
    let body = env_flag_build_content("RUST_CURSE_RUSTDOC");
    assert_eq!(
        body,
        "println!(\"cargo:rerun-if-env-changed=RUST_CURSE_RUSTDOC\");\n\
         println!(\"cargo:rustc-check-cfg=cfg(rust_curse_rustdoc)\");\n\
         if std::env::var(\"RUST_CURSE_RUSTDOC\").is_ok() {\n\
         \x20   println!(\"cargo:rustc-cfg=rust_curse_rustdoc\");\n\
         }\n",
    );
}

#[test]
fn docstring_example_matches_byte_for_byte() {
    let mut switch = BuildSwitch::new();
    switch
        .set_branch(
            "default",
            BuildBranch::new("println!(\"cargo:rerun-if-changed=src\");"),
        )
        .set_branch(
            "napi",
            BuildBranch::new("use napi_build;\nnapi_build::setup();")
                .requires_cargo_feature("node"),
        );
    let build_rs = render_build(&switch).unwrap();
    assert_eq!(
        build_rs,
        "fn main() {\n\
         \x20   {\n\
         \x20       println!(\"cargo:rerun-if-changed=src\");\n\
         \x20   }\n\
         \n\
         \x20   #[cfg(feature = \"node\")]\n\
         \x20   {\n\
         \x20       use napi_build;\n\
         \x20       napi_build::setup();\n\
         \x20   }\n\
         }\n",
    );
}

#[test]
fn write_build_writes_single_file_to_disk() {
    let dir = tempfile::tempdir().unwrap();
    let mut switch = BuildSwitch::new();
    switch
        .set_branch("default", BuildBranch::new("// default body\n"))
        .set_branch(
            "napi",
            BuildBranch::new("use napi_build;\nnapi_build::setup();\n")
                .requires_cargo_feature("node"),
        );
    write_build(dir.path(), &switch).unwrap();
    let on_disk = fs::read_to_string(dir.path().join("build.rs")).unwrap();
    assert_eq!(on_disk, render_build(&switch).unwrap());
}

#[test]
fn write_build_is_noop_when_empty() {
    let dir = tempfile::tempdir().unwrap();
    write_build(dir.path(), &BuildSwitch::new()).unwrap();
    assert!(!dir.path().join("build.rs").exists());
}
