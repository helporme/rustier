use std::fs;

use rust_curse::sync::write_lib::{
    render_lib, write_lib, LibBranch, LibSwitch, RenderLibError, RenderedFile,
};

/// Render `switch` twice and assert idempotency, then return a
/// snapshot-friendly markdown report bundling the input switch and the
/// rendered files.
fn apply(switch: &LibSwitch) -> String {
    let once = render_lib(switch).unwrap();
    let twice = render_lib(switch).unwrap();
    assert_eq!(
        once.as_deref().map(files_to_string),
        twice.as_deref().map(files_to_string),
        "render must be idempotent",
    );
    let mut report = format!("## Switch\n\n```\n{switch}```\n\n## Output\n\n");
    let Some(mut sorted) = once else {
        report.push_str("(none)\n");
        return report;
    };
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    for f in &sorted {
        report.push_str(&format!(
            "### {}.rs\n\n```rust\n{}```\n\n",
            f.name, f.content
        ));
    }
    report
}

fn files_to_string(files: &[RenderedFile]) -> String {
    let mut sorted: Vec<&RenderedFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    sorted
        .iter()
        .map(|f| format!("{}={}", f.name, f.content))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn empty_switch() {
    insta::assert_snapshot!(apply(&LibSwitch::new()));
}

#[test]
fn lib_branch_only() {
    let mut switch = LibSwitch::new();
    switch.set_branch("lib", LibBranch::new("// always included\n"));
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_or_no_reqs() {
    let mut switch = LibSwitch::new();
    switch.set_branch("extras", LibBranch::new("// extras\n"));
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_or_with_cargo_feature() {
    let mut switch = LibSwitch::new();
    switch.set_branch(
        "node",
        LibBranch::new("// node\n").requires_cargo_feature("node"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_or_with_env_flag() {
    let mut switch = LibSwitch::new();
    switch.set_branch(
        "rustdoc",
        LibBranch::new("// rustdoc\n").requires_env_flag("rust_curse_rustdoc"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_or_with_multiple_cargo_features() {
    let mut switch = LibSwitch::new();
    switch.set_branch(
        "combo",
        LibBranch::new("// combo\n")
            .requires_cargo_feature("a")
            .requires_cargo_feature("b"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_or_with_mixed_reqs() {
    let mut switch = LibSwitch::new();
    switch.set_branch(
        "mixed",
        LibBranch::new("// mixed\n")
            .requires_cargo_feature("node")
            .requires_env_flag("rust_curse_extra"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_xor_with_cargo_feature() {
    let mut switch = LibSwitch::new();
    switch.set_xor_branch(
        "rustdoc",
        LibBranch::new("// rustdoc\n").requires_cargo_feature("rustdoc"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn xor_with_or_branches() {
    let mut switch = LibSwitch::new();
    switch
        .set_branch("lib", LibBranch::new("// always included"))
        .set_branch(
            "feature_x",
            LibBranch::new("// feature-x\n").requires_cargo_feature("feature-x"),
        )
        .set_xor_branch(
            "feature_y",
            LibBranch::new("// feature-y\n").requires_cargo_feature("feature-y"),
        );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn lib_plus_or_plus_xor() {
    let mut switch = LibSwitch::new();
    switch
        .set_branch("lib", LibBranch::new("// prelude\n"))
        .set_branch("link", LibBranch::new("// link\n"))
        .set_branch(
            "napi",
            LibBranch::new("// napi\n").requires_cargo_feature("node"),
        )
        .set_xor_branch(
            "rustdoc",
            LibBranch::new("// rustdoc\n").requires_env_flag("rust_curse_rustdoc"),
        );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn multiple_or_branches_sorted() {
    let mut switch = LibSwitch::new();
    switch
        .set_branch("zeta", LibBranch::new("// zeta\n"))
        .set_branch("alpha", LibBranch::new("// alpha\n"))
        .set_branch("middle", LibBranch::new("// middle\n"));
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn multiple_xor_branches() {
    let mut switch = LibSwitch::new();
    switch
        .set_branch("link", LibBranch::new("// link\n"))
        .set_xor_branch(
            "rustdoc",
            LibBranch::new("// rustdoc\n").requires_env_flag("rust_curse_rustdoc"),
        )
        .set_xor_branch(
            "wasm",
            LibBranch::new("// wasm\n").requires_cargo_feature("wasm"),
        );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn docstring_example_matches_byte_for_byte() {
    let mut switch = LibSwitch::new();
    switch
        .set_branch("lib", LibBranch::new("// always included"))
        .set_branch(
            "feature_x",
            LibBranch::new("// feature-x\n").requires_cargo_feature("feature-x"),
        )
        .set_xor_branch(
            "feature_y",
            LibBranch::new("// feature-y\n").requires_cargo_feature("feature-y"),
        );
    let files = render_lib(&switch).unwrap().unwrap();
    let lib = files.iter().find(|f| f.name == "lib").unwrap();
    assert_eq!(
        lib.content,
        "// always included\n\
         \n\
         #[cfg(all(feature = \"feature-x\", not(feature = \"feature-y\")))]\n\
         include!(\"feature_x.rs\");\n\
         \n\
         #[cfg(feature = \"feature-y\")]\n\
         include!(\"feature_y.rs\");\n",
    );
    let feature_x = files.iter().find(|f| f.name == "feature_x").unwrap();
    assert_eq!(feature_x.content, "// feature-x\n");
    let feature_y = files.iter().find(|f| f.name == "feature_y").unwrap();
    assert_eq!(feature_y.content, "// feature-y\n");
}

#[test]
fn write_lib_writes_files_to_disk() {
    let dir = tempfile::tempdir().unwrap();
    let mut switch = LibSwitch::new();
    switch
        .set_branch("lib", LibBranch::new("// prelude\n"))
        .set_branch(
            "feature_x",
            LibBranch::new("// feature-x\n").requires_cargo_feature("feature-x"),
        );
    write_lib(dir.path(), &switch).unwrap();

    let rendered = render_lib(&switch).unwrap().unwrap();
    for file in &rendered {
        let p = dir.path().join(format!("{}.rs", file.name));
        let on_disk = fs::read_to_string(&p).unwrap();
        assert_eq!(
            on_disk, file.content,
            "file {} on disk must match render_lib output",
            file.name,
        );
    }
}

#[test]
fn xor_lib_branch_is_error() {
    let mut switch = LibSwitch::new();
    switch.set_xor_branch("lib", LibBranch::new("// nope").requires_cargo_feature("z"));
    match render_lib(&switch) {
        Err(RenderLibError::LibBranchMustBeOr) => {}
        other => panic!("expected LibBranchMustBeOr, got {other:?}"),
    }
}

#[test]
fn xor_with_no_reqs_is_error() {
    let mut switch = LibSwitch::new();
    switch.set_xor_branch("foo", LibBranch::new("// foo"));
    match render_lib(&switch) {
        Err(RenderLibError::XorBranchHasNoRequirements { name }) => {
            assert_eq!(name, "foo");
        }
        other => panic!("expected XorBranchHasNoRequirements, got {other:?}"),
    }
}

#[test]
fn write_lib_is_noop_when_empty() {
    let dir = tempfile::tempdir().unwrap();
    write_lib(dir.path(), &LibSwitch::new()).unwrap();
    assert!(!dir.path().join("lib.rs").exists());
}

#[test]
fn empty_branch_is_skipped() {
    let mut switch = LibSwitch::new();
    switch.set_xor_branch(
        "rustdoc",
        LibBranch::new("").requires_env_flag("rust_curse_rustdoc"),
    );
    assert!(
        render_lib(&switch).unwrap().is_none(),
        "empty xor branch with no prelude should render nothing",
    );
}

#[test]
fn empty_branch_is_skipped_alongside_non_empty() {
    let mut switch = LibSwitch::new();
    switch
        .set_branch("real", LibBranch::new("// real\n"))
        .set_xor_branch(
            "empty",
            LibBranch::new("").requires_env_flag("rust_curse_empty"),
        );
    let files = render_lib(&switch).unwrap().unwrap();
    assert!(
        !files.iter().any(|f| f.name == "empty"),
        "empty branch must not produce a helper file",
    );
    let lib = files.iter().find(|f| f.name == "lib").unwrap();
    assert!(
        !lib.content.contains("empty"),
        "lib.rs must not include the empty branch: {}",
        lib.content,
    );
    assert!(
        !lib.content.contains("rust_curse_empty"),
        "empty xor branch must not contribute to suppression: {}",
        lib.content,
    );
    insta::assert_snapshot!(apply(&switch));
}
