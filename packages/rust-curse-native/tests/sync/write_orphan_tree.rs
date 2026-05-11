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

    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "greet",
                rel_rs_path: "src/greet.rs",
                fns: &[FnFixture {
                    name: "hello",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "unused",
                rel_rs_path: "src/unused.rs",
                fns: &[FnFixture {
                    name: "unused_fn",
                    exposed: false,
                }],
            },
        ],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn emits_mod_decl_but_no_reexports_when_nothing_exposed() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "greet",
            rel_rs_path: "src/greet.rs",
            fns: &[FnFixture {
                name: "hello",
                exposed: false,
            }],
        }],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
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

    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "math",
            rel_rs_path: "src/math.rs",
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

    let (ir, exposed) = make_ir(
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

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
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

    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "MyMod::something",
            rel_rs_path: "src/MyMod/something.rs",
            fns: &[FnFixture {
                name: "do_it",
                exposed: true,
            }],
        }],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
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

    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "abc",
                rel_rs_path: "src/abc.rs",
                fns: &[FnFixture {
                    name: "f",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "abc::hi",
                rel_rs_path: "src/abc/hi.rs",
                fns: &[FnFixture {
                    name: "g",
                    exposed: true,
                }],
            },
        ],
    );

    let err = syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
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
        let (ir, exposed) = make_ir(
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
        syncer
            .write_orphan_tree(WriteOrphanTreeOpts {
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

    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "a::b::c",
            rel_rs_path: "src/a/b/c.rs",
            fns: &[FnFixture {
                name: "go",
                exposed: true,
            }],
        }],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
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

    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "nested::a",
                rel_rs_path: "src/nested/a.rs",
                fns: &[FnFixture {
                    name: "fa",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "nested::b",
                rel_rs_path: "src/nested/b.rs",
                fns: &[FnFixture {
                    name: "fb",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "nested::c",
                rel_rs_path: "src/nested/c.rs",
                fns: &[FnFixture {
                    name: "fc",
                    exposed: true,
                }],
            },
        ],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
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

    let (full_ir, full_exposed) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "a::b",
                rel_rs_path: "src/a/b.rs",
                fns: &[FnFixture {
                    name: "fb",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "a::c",
                rel_rs_path: "src/a/c.rs",
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
                ir: &full_ir,
                exposed: &full_exposed,
            })
            .unwrap();
    }
    let first = fs::read_to_string(&abs_a_rs).unwrap();
    assert!(first.contains("pub(crate) mod b;"), "first a.rs: {first:?}");
    assert!(first.contains("pub(crate) mod c;"), "first a.rs: {first:?}");

    let (reduced_ir, reduced_exposed) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "a::b",
            rel_rs_path: "src/a/b.rs",
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
                ir: &reduced_ir,
                exposed: &reduced_exposed,
            })
            .unwrap();
    }
    let second = fs::read_to_string(&abs_a_rs).unwrap();
    assert!(second.contains("pub(crate) mod b;"), "second a.rs: {second:?}");
    assert!(!second.contains("pub(crate) mod c;"), "second a.rs: {second:?}");
}

#[test]
fn flattening_module_cleans_old_subtree() {
    let project = TestProject::new();
    let abs_subtree_dir = project.abs_root_path.join(OUT_DIR).join(ORPHAN_TREE_DIR);

    {
        let mut syncer = project.syncer();
        let (ir, exposed) = make_ir(
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
        syncer
            .write_orphan_tree(WriteOrphanTreeOpts {
                ir: &ir,
                exposed: &exposed,
            })
            .unwrap();
    }
    // a is non-leaf (has child b), so a.rs subtree file is written.
    assert!(abs_subtree_dir.join("a.rs").exists());

    {
        let mut syncer = project.syncer();
        let (ir, exposed) = make_ir(
            TEST_CRATE,
            &[ModFixture {
                mod_name: "a",
                rel_rs_path: "src/a.rs",
                fns: &[FnFixture {
                    name: "f",
                    exposed: true,
                }],
            }],
        );
        syncer
            .write_orphan_tree(WriteOrphanTreeOpts {
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
    let (ir, exposed) = make_ir(
        TEST_CRATE,
        &[
            ModFixture {
                mod_name: "greet",
                rel_rs_path: "src/greet.rs",
                fns: &[FnFixture {
                    name: "hello",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "nested::deep",
                rel_rs_path: "src/nested/deep.rs",
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
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    let first = project_to_md(&project);

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    let second = project_to_md(&project);

    assert_eq!(first, second, "second call must be byte-identical");
}

#[test]
fn module_without_filename_is_skipped() {
    let project = TestProject::new();
    let mut syncer = project.syncer();

    // A `ModDef` with `span: None` (e.g. one rustdoc emitted no
    // span for) is filtered out by `write_orphan_tree`, so no
    // `pub mod` decl and no `pub use` line is emitted.
    let (mut ir, exposed) = make_ir(
        TEST_CRATE,
        &[ModFixture {
            mod_name: "ghost",
            rel_rs_path: "src/ghost.rs",
            fns: &[FnFixture {
                name: "boo",
                exposed: true,
            }],
        }],
    );
    for item in ir.items.iter_mut() {
        if let rust_curse::ir::Item::Mod(m) = item {
            if m.path.segments.last().map(String::as_str) == Some("ghost") {
                m.span = None;
            }
        }
    }

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
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
        "orphan_tree.rs must not be written when no module has a filename: {}",
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
            rel_rs_path: "src/greet.rs",
            fns: &[FnFixture {
                name: "hello",
                exposed: true,
            }],
        }],
    );

    syncer
        .write_orphan_tree(WriteOrphanTreeOpts {
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    syncer.write_rustdoc(&modules).unwrap();

    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}
