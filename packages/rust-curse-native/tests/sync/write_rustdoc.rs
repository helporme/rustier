use std::fs;

use rust_curse::sync::write_rustdoc::{
    WriteRustdocError, RUSTDOC_CFG_FLAG, RUSTDOC_DIR, RUSTDOC_ENV_VAR,
};

use crate::common::{project_to_md, test_module, TestProject, OUT_DIR};

#[test]
fn empty_modules() {
    let project = TestProject::new();
    let mut syncer = project.syncer();
    syncer.write_rustdoc(&[]).unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn flat_modules_emit_pub_mod_tree() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let modules = vec![
        test_module("greet", "src/greet.rs"),
        test_module("math", "src/math.rs"),
    ];
    syncer.write_rustdoc(&modules).unwrap();

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn nested_modules_emit_subtree_files() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let modules = vec![
        test_module("nested::deep", "src/nested/deep.rs"),
        test_module("nested::shallow", "src/nested/shallow.rs"),
    ];
    syncer.write_rustdoc(&modules).unwrap();

    let subtree_dir = project.abs_root_path.join(OUT_DIR).join(RUSTDOC_DIR);
    assert!(subtree_dir.join("nested.rs").exists());

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn uppercase_segment_gets_allow() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let modules = vec![test_module("MyMod::something", "src/MyMod/something.rs")];
    syncer.write_rustdoc(&modules).unwrap();

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn leaf_branch_collision_errors() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let modules = vec![
        test_module("abc", "src/abc.rs"),
        test_module("abc::hi", "src/abc/hi.rs"),
    ];

    let err = syncer.write_rustdoc(&modules).unwrap_err();
    match err {
        WriteRustdocError::Tree(e) => {
            assert!(
                matches!(
                    e,
                    rust_curse::sync::tree::TreeError::LeafBranchCollision { ref name } if name == "abc"
                ),
                "expected LeafBranchCollision for 'abc', got {e:?}"
            );
        }
        other => panic!("expected Tree error, got {other:?}"),
    }
}

#[test]
fn stale_subtree_cleaned_on_rerun() {
    let project = TestProject::new();
    let abs_subtree_dir = project.abs_root_path.join(OUT_DIR).join(RUSTDOC_DIR);

    {
        let mut syncer = project.syncer();
        let modules = vec![
            test_module("nested::deep", "src/nested/deep.rs"),
            test_module("nested::shallow", "src/nested/shallow.rs"),
        ];
        syncer.write_rustdoc(&modules).unwrap();
    }
    assert!(abs_subtree_dir.join("nested.rs").exists());

    {
        let mut syncer = project.syncer();
        syncer.write_rustdoc(&[]).unwrap();
    }
    assert!(!abs_subtree_dir.join("nested.rs").exists());
}

#[test]
fn build_rs_emits_env_cfg_directive() {
    let project = TestProject::new();
    let mut syncer = project.syncer();
    syncer.write_rustdoc(&[]).unwrap();

    let build_rs =
        fs::read_to_string(project.abs_root_path.join(OUT_DIR).join("build.rs")).unwrap();
    assert!(
        build_rs.contains(&format!("cargo:rerun-if-env-changed={RUSTDOC_ENV_VAR}")),
        "build.rs missing rerun-if-env-changed: {build_rs}"
    );
    assert!(
        build_rs.contains(&format!("cargo:rustc-cfg={RUSTDOC_CFG_FLAG}")),
        "build.rs missing rustc-cfg: {build_rs}"
    );

    // Lock the structure with a snapshot so further changes are deliberate.
    insta::assert_snapshot!("build_rs_directive_body", build_rs);
}

#[test]
fn idempotent_when_called_twice_with_same_inputs() {
    let project = TestProject::new();
    let modules = vec![
        test_module("greet", "src/greet.rs"),
        test_module("nested::deep", "src/nested/deep.rs"),
    ];

    let mut syncer = project.syncer();
    syncer.write_rustdoc(&modules).unwrap();
    let first = project_to_md(&project);

    syncer.write_rustdoc(&modules).unwrap();
    let second = project_to_md(&project);

    assert_eq!(first, second, "second call must be byte-identical");
}

