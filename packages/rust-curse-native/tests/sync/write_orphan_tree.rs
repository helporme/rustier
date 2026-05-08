use std::fs;

use rust_curse::sync::write_orphan_tree::{
    WriteOrphanTreeError, WriteOrphanTreeOpts, ORPHAN_TREE_DIR,
};

use crate::common::{
    empty_ir, make_ir, project_to_md, test_module, FnFixture, ModFixture, TestProject, OUT_DIR,
};

const TEST_CRATE: &str = "test_crate";

#[test]
fn empty_modules() {
    let project = TestProject::new();
    let mut syncer = project.syncer();
    let (ir, exposed) = empty_ir(TEST_CRATE);
    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &[],
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn emits_mod_tree_and_reexports() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let modules = vec![
        test_module("greet", "src/greet.rs"),
        test_module("unused", "src/unused.rs"),
    ];
    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "greet",
                fns: &[FnFixture {
                    name: "hello",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "unused",
                fns: &[FnFixture {
                    name: "unused_fn",
                    exposed: false,
                }],
            },
        ],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn no_tree_or_reexports_when_nothing_exposed() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let modules = vec![test_module("greet", "src/greet.rs")];
    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "greet",
            fns: &[FnFixture {
                name: "hello",
                exposed: false,
            }],
        }],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn reexports_all_exposed_items() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let modules = vec![test_module("math", "src/math.rs")];
    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "math",
            fns: &[
                FnFixture {
                    name: "add",
                    exposed: true,
                },
                FnFixture {
                    name: "sub",
                    exposed: true,
                },
            ],
        }],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
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
    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "nested::deep",
                fns: &[FnFixture {
                    name: "run",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "nested::shallow",
                fns: &[FnFixture {
                    name: "skim",
                    exposed: true,
                }],
            },
        ],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    assert!(project
        .abs_root_path
        .join(OUT_DIR)
        .join(ORPHAN_TREE_DIR)
        .join("nested.rs")
        .exists());
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn uppercase_segment_gets_allow() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let modules = vec![test_module("MyMod::something", "src/MyMod/something.rs")];
    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "MyMod::something",
            fns: &[FnFixture {
                name: "do_it",
                exposed: true,
            }],
        }],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
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
    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "abc",
                fns: &[FnFixture {
                    name: "f",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "abc::hi",
                fns: &[FnFixture {
                    name: "g",
                    exposed: true,
                }],
            },
        ],
    );

    let err = syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap_err();
    match err {
        WriteOrphanTreeError::Tree(e) => {
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
    let abs_subtree_dir = project.abs_root_path.join(OUT_DIR).join(ORPHAN_TREE_DIR);

    {
        let mut syncer = project.syncer();
        let modules = vec![test_module("a::b", "src/a/b.rs")];
        let (ir, exposed) = make_ir(
            TEST_CRATE,
            &[ModFixture {
                mod_name: "a::b",
                fns: &[FnFixture {
                    name: "f",
                    exposed: true,
                }],
            }],
        );
        syncer
            .write_orphan_tree(WriteOrphanTreeOpts {
                modules: &modules,
                ir: &ir,
                exposed: &exposed,
            })
            .unwrap();
    }
    assert!(abs_subtree_dir.join("a.rs").exists());

    {
        let mut syncer = project.syncer();
        let (ir, exposed) = empty_ir(TEST_CRATE);
        syncer
            .write_orphan_tree(WriteOrphanTreeOpts {
                modules: &[],
                ir: &ir,
                exposed: &exposed,
            })
            .unwrap();
    }
    assert!(!abs_subtree_dir.join("a.rs").exists());
}

#[test]
fn deep_nesting_three_levels() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let modules = vec![test_module("a::b::c", "src/a/b/c.rs")];
    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "a::b::c",
            fns: &[FnFixture {
                name: "go",
                exposed: true,
            }],
        }],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();

    let subtree_dir = project.abs_root_path.join(OUT_DIR).join(ORPHAN_TREE_DIR);
    assert!(subtree_dir.join("a.rs").exists());
    assert!(subtree_dir.join("a").join("b.rs").exists());

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn siblings_share_parent_subtree_file() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let modules = vec![
        test_module("nested::a", "src/nested/a.rs"),
        test_module("nested::b", "src/nested/b.rs"),
        test_module("nested::c", "src/nested/c.rs"),
    ];
    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "nested::a",
                fns: &[FnFixture {
                    name: "fa",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "nested::b",
                fns: &[FnFixture {
                    name: "fb",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "nested::c",
                fns: &[FnFixture {
                    name: "fc",
                    exposed: true,
                }],
            },
        ],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn partial_module_reduction_regenerates_parent_subtree_file() {
    let project = TestProject::new();
    let abs_a_rs = project
        .abs_root_path
        .join(OUT_DIR)
        .join(ORPHAN_TREE_DIR)
        .join("a.rs");

    let full_modules = vec![
        test_module("a::b", "src/a/b.rs"),
        test_module("a::c", "src/a/c.rs"),
    ];
    let (full_ir, full_exposed) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "a::b",
                fns: &[FnFixture {
                    name: "fb",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "a::c",
                fns: &[FnFixture {
                    name: "fc",
                    exposed: true,
                }],
            },
        ],
    );

    {
        let mut syncer = project.syncer();
        syncer
            .write_orphan_tree(WriteOrphanTreeOpts {
                modules: &full_modules,
                ir: &full_ir,
                exposed: &full_exposed,
            })
            .unwrap();
    }
    let first = fs::read_to_string(&abs_a_rs).unwrap();
    assert!(first.contains("pub mod b;"), "first a.rs: {first:?}");
    assert!(first.contains("pub mod c;"), "first a.rs: {first:?}");

    let reduced_modules = vec![test_module("a::b", "src/a/b.rs")];
    let (reduced_ir, reduced_exposed) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "a::b",
            fns: &[FnFixture {
                name: "fb",
                exposed: true,
            }],
        }],
    );
    {
        let mut syncer = project.syncer();
        syncer
            .write_orphan_tree(WriteOrphanTreeOpts {
                modules: &reduced_modules,
                ir: &reduced_ir,
                exposed: &reduced_exposed,
            })
            .unwrap();
    }
    let second = fs::read_to_string(&abs_a_rs).unwrap();
    assert!(second.contains("pub mod b;"), "second a.rs: {second:?}");
    assert!(!second.contains("pub mod c;"), "second a.rs: {second:?}");
}

#[test]
fn flattening_module_cleans_old_subtree() {
    let project = TestProject::new();
    let abs_subtree_dir = project.abs_root_path.join(OUT_DIR).join(ORPHAN_TREE_DIR);

    {
        let mut syncer = project.syncer();
        let modules = vec![test_module("a::b", "src/a/b.rs")];
        let (ir, exposed) = make_ir(
            TEST_CRATE,
            &[ModFixture {
                mod_name: "a::b",
                fns: &[FnFixture {
                    name: "f",
                    exposed: true,
                }],
            }],
        );
        syncer
            .write_orphan_tree(WriteOrphanTreeOpts {
                modules: &modules,
                ir: &ir,
                exposed: &exposed,
            })
            .unwrap();
    }
    // a is non-leaf (has child b), so a.rs subtree file is written.
    assert!(abs_subtree_dir.join("a.rs").exists());

    {
        let mut syncer = project.syncer();
        let modules = vec![test_module("a", "src/a.rs")];
        let (ir, exposed) = make_ir(
            TEST_CRATE,
            &[ModFixture {
                mod_name: "a",
                fns: &[FnFixture {
                    name: "f",
                    exposed: true,
                }],
            }],
        );
        syncer
            .write_orphan_tree(WriteOrphanTreeOpts {
                modules: &modules,
                ir: &ir,
                exposed: &exposed,
            })
            .unwrap();
    }
    // a is now a leaf pointing at user source; the old subtree file
    // must be wiped.
    assert!(!abs_subtree_dir.join("a.rs").exists());
}

#[test]
fn idempotent_when_called_twice_with_same_inputs() {
    let project = TestProject::new();
    let modules = vec![
        test_module("greet", "src/greet.rs"),
        test_module("nested::deep", "src/nested/deep.rs"),
    ];
    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "greet",
                fns: &[FnFixture {
                    name: "hello",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "nested::deep",
                fns: &[FnFixture {
                    name: "go",
                    exposed: true,
                }],
            },
        ],
    );

    let mut syncer = project.syncer();
    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    let first = project_to_md(&project);

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    let second = project_to_md(&project);

    assert_eq!(first, second, "second call must be byte-identical");
}

#[test]
fn module_in_modules_but_missing_from_ir_is_skipped() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    // `ghost` appears in `modules` but the IR is empty so it has no
    // ModDef. The current contract: such modules are filtered out by
    // `module_has_exposed_export` before reaching the tree builder, so
    // no `pub mod ghost;` decl and no `pub use` line is emitted.
    let modules = vec![test_module("ghost", "src/ghost.rs")];
    let (ir, exposed) = empty_ir(TEST_CRATE);

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();

    let abs_orphan_tree_rs = project
        .abs_root_path
        .join(OUT_DIR)
        .join("orphan_tree.rs");
    assert!(
        !abs_orphan_tree_rs.exists(),
        "orphan_tree.rs must not be written when no module has an exposed export: {}",
        project_to_md(&project),
    );
}

#[test]
fn rustdoc_xor_branch_suppresses_orphan_tree_in_lib_rs() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let modules = vec![test_module("greet", "src/greet.rs")];
    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "greet",
            fns: &[FnFixture {
                name: "hello",
                exposed: true,
            }],
        }],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    syncer.write_rustdoc(&modules).unwrap();

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}
