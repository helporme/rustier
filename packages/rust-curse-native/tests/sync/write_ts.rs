use rust_curse::sync::write_ts::{TS_AMBIENT_FILE, TS_TYPES_DIR};

use crate::common::{
    empty_ir, make_ir, project_to_md, FnFixture, ModFixture, TestProject, OUT_DIR,
};

const TEST_CRATE: &str = "test_crate";

#[test]
fn empty_ir_writes_only_ambient() {
    let project = TestProject::new();
    let mut syncer = project.syncer();
    let (ir, _) = empty_ir(TEST_CRATE);
    syncer.write_ts(&ir).unwrap();

    assert!(project.abs_root_path.join(OUT_DIR).join(TS_AMBIENT_FILE).exists());
    assert!(!project.abs_root_path.join(OUT_DIR).join(TS_TYPES_DIR).exists());

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn single_flat_module() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let (ir, _) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "foo",
            rel_rs_path: "src/foo.rs",
            fns: &[FnFixture {
                name: "f",
                exposed: true,
            }],
        }],
    );

    syncer.write_ts(&ir).unwrap();

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn multiple_flat_modules() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let (ir, _) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "foo",
                rel_rs_path: "src/foo.rs",
                fns: &[FnFixture {
                    name: "f",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "bar",
                rel_rs_path: "src/bar.rs",
                fns: &[FnFixture {
                    name: "g",
                    exposed: true,
                }],
            },
        ],
    );

    syncer.write_ts(&ir).unwrap();

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn nested_modules() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let (ir, _) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "nested::deep",
                rel_rs_path: "src/nested/deep.rs",
                fns: &[FnFixture {
                    name: "run",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "nested::shallow",
                rel_rs_path: "src/nested/shallow.rs",
                fns: &[FnFixture {
                    name: "skim",
                    exposed: true,
                }],
            },
        ],
    );

    syncer.write_ts(&ir).unwrap();

    let abs_types_dir = project.abs_root_path.join(OUT_DIR).join(TS_TYPES_DIR);
    assert!(abs_types_dir.join("nested/deep.rs.d.ts").exists());
    assert!(abs_types_dir.join("nested/shallow.rs.d.ts").exists());
    assert!(
        !abs_types_dir.join("nested.rs.d.ts").exists(),
        "no intermediate .rs.d.ts should be emitted for non-leaf segments"
    );

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn deeply_nested_module() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let (ir, _) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "a::b::c",
            rel_rs_path: "src/a/b/c.rs",
            fns: &[FnFixture {
                name: "f",
                exposed: true,
            }],
        }],
    );

    syncer.write_ts(&ir).unwrap();

    assert!(project
        .abs_root_path
        .join(OUT_DIR)
        .join(TS_TYPES_DIR)
        .join("a/b/c.rs.d.ts")
        .exists());

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn uppercase_segment_preserved() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let (ir, _) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "MyMod::sub",
            rel_rs_path: "src/MyMod/sub.rs",
            fns: &[FnFixture {
                name: "do_it",
                exposed: true,
            }],
        }],
    );

    syncer.write_ts(&ir).unwrap();

    assert!(project
        .abs_root_path
        .join(OUT_DIR)
        .join(TS_TYPES_DIR)
        .join("MyMod/sub.rs.d.ts")
        .exists());

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn stale_subtree_cleaned_on_rerun() {
    let project = TestProject::new();
    let abs_types_dir = project.abs_root_path.join(OUT_DIR).join(TS_TYPES_DIR);
    let abs_ambient = project.abs_root_path.join(OUT_DIR).join(TS_AMBIENT_FILE);

    {
        let mut syncer = project.syncer();
        let (ir, _) = make_ir(
            TEST_CRATE,
            &[ModFixture {
                mod_name: "a::b",
                rel_rs_path: "src/a/b.rs",
                fns: &[FnFixture {
                    name: "f",
                    exposed: true,
                }],
            }],
        );
        syncer.write_ts(&ir).unwrap();
    }
    assert!(abs_types_dir.join("a/b.rs.d.ts").exists());

    {
        let mut syncer = project.syncer();
        let (ir, _) = empty_ir(TEST_CRATE);
        syncer.write_ts(&ir).unwrap();
    }
    assert!(!abs_types_dir.exists(), "types/ should be wiped on rerun");
    assert!(abs_ambient.exists(), "ambient.d.ts should still be present");
}

