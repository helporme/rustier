use std::fs;

use rust_curse::sync::write_lib::{
    render_lib, write_lib, Lib, LibSwitch, RenderLibError, RenderedFile,
};

/// Render `switch` twice and assert idempotency, then return a
/// snapshot-friendly markdown report bundling the input switch and the
/// rendered files.
fn apply(switch: &LibSwitch) -> String {
    let once = render_lib(switch).unwrap();
    let twice = render_lib(switch).unwrap();
    assert_eq!(
        files_to_string(&once),
        files_to_string(&twice),
        "render must be idempotent",
    );
    let mut sorted = once;
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let mut report = format!("## Switch\n\n```\n{switch}```\n\n## Output\n\n");
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
    let switch = LibSwitch::new().branch("lib", Lib::new("// always included\n"));
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_or_no_reqs() {
    let switch = LibSwitch::new().branch("extras", Lib::new("// extras\n"));
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_or_with_cargo_feature() {
    let switch = LibSwitch::new().branch(
        "node",
        Lib::new("// node\n").requires_cargo_feature("node"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_or_with_env_flag() {
    let switch = LibSwitch::new().branch(
        "rustdoc",
        Lib::new("// rustdoc\n").requires_env_flag("rust_curse_rustdoc"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_or_with_multiple_cargo_features() {
    let switch = LibSwitch::new().branch(
        "combo",
        Lib::new("// combo\n")
            .requires_cargo_feature("a")
            .requires_cargo_feature("b"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_or_with_mixed_reqs() {
    let switch = LibSwitch::new().branch(
        "mixed",
        Lib::new("// mixed\n")
            .requires_cargo_feature("node")
            .requires_env_flag("rust_curse_extra"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn single_xor_with_cargo_feature() {
    let switch = LibSwitch::new().xor_branch(
        "rustdoc",
        Lib::new("// rustdoc\n").requires_cargo_feature("rustdoc"),
    );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn xor_with_or_branches() {
    let switch = LibSwitch::new()
        .branch("lib", Lib::new("// always included"))
        .branch(
            "feature_x",
            Lib::new("// feature-x\n").requires_cargo_feature("feature-x"),
        )
        .xor_branch(
            "feature_y",
            Lib::new("// feature-y\n").requires_cargo_feature("feature-y"),
        );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn lib_plus_or_plus_xor() {
    let switch = LibSwitch::new()
        .branch("lib", Lib::new("// prelude\n"))
        .branch("link", Lib::new("// link\n"))
        .branch(
            "napi",
            Lib::new("// napi\n").requires_cargo_feature("node"),
        )
        .xor_branch(
            "rustdoc",
            Lib::new("// rustdoc\n").requires_env_flag("rust_curse_rustdoc"),
        );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn multiple_or_branches_sorted() {
    let switch = LibSwitch::new()
        .branch("zeta", Lib::new("// zeta\n"))
        .branch("alpha", Lib::new("// alpha\n"))
        .branch("middle", Lib::new("// middle\n"));
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn multiple_xor_branches() {
    let switch = LibSwitch::new()
        .branch("link", Lib::new("// link\n"))
        .xor_branch(
            "rustdoc",
            Lib::new("// rustdoc\n").requires_env_flag("rust_curse_rustdoc"),
        )
        .xor_branch(
            "wasm",
            Lib::new("// wasm\n").requires_cargo_feature("wasm"),
        );
    insta::assert_snapshot!(apply(&switch));
}

#[test]
fn docstring_example_matches_byte_for_byte() {
    let switch = LibSwitch::new()
        .branch("lib", Lib::new("// always included"))
        .branch(
            "feature_x",
            Lib::new("// feature-x\n").requires_cargo_feature("feature-x"),
        )
        .xor_branch(
            "feature_y",
            Lib::new("// feature-y\n").requires_cargo_feature("feature-y"),
        );
    let files = render_lib(&switch).unwrap();
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
    let switch = LibSwitch::new()
        .branch("lib", Lib::new("// prelude\n"))
        .branch(
            "feature_x",
            Lib::new("// feature-x\n").requires_cargo_feature("feature-x"),
        );
    write_lib(dir.path(), &switch).unwrap();

    let rendered = render_lib(&switch).unwrap();
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
    let switch =
        LibSwitch::new().xor_branch("lib", Lib::new("// nope").requires_cargo_feature("z"));
    match render_lib(&switch) {
        Err(RenderLibError::LibBranchMustBeOr) => {}
        other => panic!("expected LibBranchMustBeOr, got {other:?}"),
    }
}

#[test]
fn xor_with_no_reqs_is_error() {
    let switch = LibSwitch::new().xor_branch("foo", Lib::new("// foo"));
    match render_lib(&switch) {
        Err(RenderLibError::XorBranchHasNoRequirements { name }) => {
            assert_eq!(name, "foo");
        }
        other => panic!("expected XorBranchHasNoRequirements, got {other:?}"),
    }
}
