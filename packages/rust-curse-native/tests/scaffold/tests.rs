use std::fs;
use std::path::PathBuf;

use rust_curse::orphans::OrphanModule;
use rust_curse::scaffold::link::{LinkSectionError, LinkSectionOpts, LINK_DIR};
use rust_curse::scaffold::napi::NapiSectionOpts;
use rust_curse::scaffold::rustdoc::{RustdocSectionError, RustdocSectionOpts, RUSTDOC_DIR};
use rust_curse::scaffold::ts::TS_FILE;
use rust_curse::scaffold::wasm::WasmSectionOpts;
use rust_curse::scaffold::{BuildContribution, ScaffoldError, ScaffoldSection};

use crate::common::{
    empty_ir, make_ir, project_to_md, FnFixture, ModFixture, TestProject, WORKSPACE_CARGO_TOML,
};

const TEST_CRATE: &str = "test_crate";

fn test_module(mod_name: &str, rel_rs_path: &str) -> OrphanModule {
    OrphanModule {
        rel_rs_path: PathBuf::from(rel_rs_path),
        mod_name: mod_name.to_string(),
    }
}

#[test]
fn set_link_only() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let (ir, exposed) = empty_ir(TEST_CRATE);
    scaffold
        .set_link(LinkSectionOpts {
            modules: &[],
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn link_emits_mod_tree_and_reexports() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

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

    scaffold
        .set_link(LinkSectionOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn link_no_tree_or_reexports_when_nothing_exposed() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

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

    scaffold
        .set_link(LinkSectionOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn link_reexports_all_exposed_items() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

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

    scaffold
        .set_link(LinkSectionOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn link_nested_modules_emit_subtree_files() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

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

    scaffold
        .set_link(LinkSectionOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    scaffold.deploy().unwrap();
    assert!(project.abs_temp_path.join(LINK_DIR).join("nested.rs").exists());
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

/// link is always-applied with no activation; combined with napi
/// (cargo-feature gated) and no exclusive section, link's include should
/// have no `#[cfg]` attr and napi's should be `#[cfg(feature = "node")]`.
#[test]
fn link_with_napi() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

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

    scaffold
        .set_link(LinkSectionOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    scaffold
        .set_napi(NapiSectionOpts {
            content: "// napi glue\n".into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

/// link + rustdoc: rustdoc uses exclusive_env_flag, so link's include
/// must be gated by `#[cfg(not(rust_curse_rustdoc))]` and rustdoc's by
/// `#[cfg(rust_curse_rustdoc)]`.
#[test]
fn link_with_rustdoc_exclusive() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

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

    scaffold
        .set_link(LinkSectionOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    scaffold
        .set_rustdoc(RustdocSectionOpts {
            modules: &[test_module("greet", "src/greet.rs")],
        })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

/// link + napi + wasm: link is always-on, both targets are
/// cargo-feature-gated and additive. Verifies the bug fix — wasm glue
/// can resolve `crate::math::add` because link provides the tree.
#[test]
fn link_with_napi_and_wasm() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

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

    scaffold
        .set_link(LinkSectionOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    scaffold
        .set_napi(NapiSectionOpts {
            content: "// napi glue\n".into(),
        })
        .unwrap();
    scaffold
        .set_wasm(WasmSectionOpts {
            content: "// wasm glue\n".into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn set_napi_only() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set_napi(NapiSectionOpts {
            content: "// napi glue\n".into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn set_wasm_only() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set_wasm(WasmSectionOpts {
            content: "// wasm glue\n".into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn set_ts_only() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let (ir, _) = empty_ir(TEST_CRATE);
    scaffold.set_ts(&ir).unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn set_ts_writes_ambient_dts() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let (ir, _) = empty_ir(TEST_CRATE);
    scaffold.set_ts(&ir).unwrap();
    scaffold.deploy().unwrap();
    let written = fs::read_to_string(project.abs_temp_path.join(TS_FILE)).unwrap();
    assert_eq!(written, ir.print_ts_ambient());
}

#[test]
fn set_ts_combined_with_napi() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let (ir, _) = empty_ir(TEST_CRATE);
    scaffold
        .set_napi(NapiSectionOpts {
            content: "// napi glue\n".into(),
        })
        .unwrap();
    scaffold.set_ts(&ir).unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn remove_ts_clears_ambient_dts() {
    let project = TestProject::new();
    let (ir, _) = empty_ir(TEST_CRATE);
    {
        let mut scaffold = project.scaffold();
        scaffold.set_ts(&ir).unwrap();
        scaffold.deploy().unwrap();
    }
    assert!(project.abs_temp_path.join(TS_FILE).exists());
    {
        let mut scaffold = project.scaffold();
        scaffold.deploy().unwrap();
    }
    assert!(
        !project.abs_temp_path.join(TS_FILE).exists(),
        "ambient.d.ts should be removed when ts section is dropped"
    );
}

#[test]
fn set_rustdoc_only() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set_rustdoc(RustdocSectionOpts { modules: &[] })
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn rustdoc_renders_simple_modules() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let modules = vec![
        test_module("greet", "greet.rs"),
        test_module("math", "math.rs"),
    ];
    scaffold
        .set_rustdoc(RustdocSectionOpts { modules: &modules })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn rustdoc_emits_nested_subtree() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let modules = vec![test_module("nested::deep", "nested/deep.rs")];
    scaffold
        .set_rustdoc(RustdocSectionOpts { modules: &modules })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn rustdoc_deeply_nested_single_branch() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let modules = vec![test_module("a::b::c::d", "src/a/b/c/d.rs")];
    scaffold
        .set_rustdoc(RustdocSectionOpts { modules: &modules })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn rustdoc_siblings_share_parent_file() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let modules = vec![
        test_module("abc::hi", "src/abc/hi.rs"),
        test_module("abc::bye", "src/abc/bye.rs"),
    ];
    scaffold
        .set_rustdoc(RustdocSectionOpts { modules: &modules })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn rustdoc_multiple_deep_branches() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let modules = vec![
        test_module("a::b::c", "src/a/b/c.rs"),
        test_module("a::b::d", "src/a/b/d.rs"),
        test_module("a::e::f", "src/a/e/f.rs"),
    ];
    scaffold
        .set_rustdoc(RustdocSectionOpts { modules: &modules })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn rustdoc_mixes_flat_and_nested_at_root() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let modules = vec![
        test_module("kitty", "src/kitty.rs"),
        test_module("abc::hi", "src/abc/hi.rs"),
    ];
    scaffold
        .set_rustdoc(RustdocSectionOpts { modules: &modules })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn rustdoc_uppercase_segment_gets_allow() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let modules = vec![test_module("MyMod::something", "src/MyMod/something.rs")];
    scaffold
        .set_rustdoc(RustdocSectionOpts { modules: &modules })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn rustdoc_leaf_branch_collision_errors() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let modules = vec![
        test_module("abc", "src/abc.rs"),
        test_module("abc::hi", "src/abc/hi.rs"),
    ];
    let err = scaffold
        .set_rustdoc(RustdocSectionOpts { modules: &modules })
        .unwrap_err();
    match err {
        RustdocSectionError::LeafBranchCollision { name } => assert_eq!(name, "abc"),
        other => panic!("expected LeafBranchCollision, got {other:?}"),
    }
}

#[test]
fn rustdoc_stale_subtree_cleaned_on_redeploy() {
    let project = TestProject::new();
    let abs_subtree_dir = project.abs_temp_path.join(RUSTDOC_DIR);
    let abs_lib_rustdoc = project.abs_temp_path.join("lib-rustdoc.rs");

    {
        let mut scaffold = project.scaffold();
        let modules = vec![test_module("a::b", "src/a/b.rs")];
        scaffold
            .set_rustdoc(RustdocSectionOpts { modules: &modules })
            .unwrap();
        scaffold.deploy().unwrap();
    }
    assert!(abs_subtree_dir.join("a.rs").exists());

    {
        let mut scaffold = project.scaffold();
        scaffold
            .set_rustdoc(RustdocSectionOpts { modules: &[] })
            .unwrap();
        scaffold.deploy().unwrap();
    }
    assert!(
        !abs_subtree_dir.exists(),
        "lib-rustdoc/ directory should be cleaned when no modules remain"
    );
    let lib_content = fs::read_to_string(&abs_lib_rustdoc).unwrap();
    assert_eq!(
        lib_content,
        "// generated by rust-curse with love <3\n// do not change manually\n\n"
    );
}

#[test]
fn link_leaf_branch_collision_errors() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
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
                    name: "x",
                    exposed: true,
                }],
            },
            ModFixture {
                mod_name: "abc::hi",
                fns: &[FnFixture {
                    name: "y",
                    exposed: true,
                }],
            },
        ],
    );
    let err = scaffold
        .set_link(LinkSectionOpts {
            modules: &modules,
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap_err();
    match err {
        LinkSectionError::LeafBranchCollision { name } => assert_eq!(name, "abc"),
        other => panic!("expected LeafBranchCollision, got {other:?}"),
    }
}

#[test]
fn set_combo_link_napi_wasm_rustdoc() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let (ir, exposed) = empty_ir(TEST_CRATE);
    scaffold
        .set_link(LinkSectionOpts {
            modules: &[],
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    scaffold
        .set_napi(NapiSectionOpts {
            content: "// napi\n".into(),
        })
        .unwrap();
    scaffold
        .set_wasm(WasmSectionOpts {
            content: "// wasm\n".into(),
        })
        .unwrap();
    scaffold
        .set_rustdoc(RustdocSectionOpts { modules: &[] })
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn second_exclusive_env_section_returns_error() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set(
            ScaffoldSection::new("rustdoc")
                .exclusive_env_flag("RUST_CURSE_RUSTDOC")
                .content("// r\n"),
        )
        .unwrap();
    let err = scaffold
        .set(
            ScaffoldSection::new("other")
                .exclusive_env_flag("RUST_CURSE_OTHER")
                .content("// o\n"),
        )
        .unwrap_err();
    match err {
        ScaffoldError::MultipleExclusiveEnv {
            existing,
            attempted,
        } => {
            assert_eq!(existing, "rustdoc");
            assert_eq!(attempted, "other");
        }
        other => panic!("expected MultipleExclusiveEnv, got {other:?}"),
    }
}

#[test]
fn replacing_exclusive_env_section_with_itself_is_allowed() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set(
            ScaffoldSection::new("rustdoc")
                .exclusive_env_flag("RUST_CURSE_RUSTDOC")
                .content("// r\n"),
        )
        .unwrap();
    scaffold
        .set(
            ScaffoldSection::new("rustdoc")
                .exclusive_env_flag("RUST_CURSE_RUSTDOC")
                .content("// r2\n"),
        )
        .unwrap();
}

#[test]
fn patch_replaces_only_content() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set_napi(NapiSectionOpts {
            content: "// original\n".into(),
        })
        .unwrap();
    scaffold
        .patch(ScaffoldSection::new("napi").content("// patched\n"))
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn patch_unknown_section_errors() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let err = scaffold
        .patch(ScaffoldSection::new("nope").content("x"))
        .unwrap_err();
    assert!(matches!(err, ScaffoldError::PatchUnknownSection(_)));
}

#[test]
fn remove_section_drops_files_and_deps() {
    let project = TestProject::new();
    {
        let mut scaffold = project.scaffold();
        scaffold
            .set_napi(NapiSectionOpts {
                content: "// napi\n".into(),
            })
            .unwrap();
        scaffold
            .set_wasm(WasmSectionOpts {
                content: "// wasm\n".into(),
            })
            .unwrap();
        scaffold.deploy().unwrap();
    }
    {
        let mut scaffold = project.scaffold();
        scaffold
            .set_wasm(WasmSectionOpts {
                content: "// wasm\n".into(),
            })
            .unwrap();
        scaffold.deploy().unwrap();
    }
    insta::assert_snapshot!(project_to_md(&project));
}

/// A section with no deps / build-deps / feature must not produce empty
/// `[features]`, `[dependencies]`, or `[build-dependencies]` tables in
/// the patched `Cargo.toml`.
#[test]
fn link_only_emits_no_empty_blocks() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let (ir, exposed) = empty_ir(TEST_CRATE);
    scaffold
        .set_link(LinkSectionOpts {
            modules: &[],
            ir: &ir,
            exposed: &exposed,
        })
        .unwrap();
    scaffold.deploy().unwrap();
    let cargo = fs::read_to_string(project.abs_root_path.join("Cargo.toml")).unwrap();
    assert!(
        !cargo.contains("[features]"),
        "unexpected [features] table:\n{cargo}"
    );
    assert!(
        !cargo.contains("[dependencies]"),
        "unexpected [dependencies] table:\n{cargo}"
    );
    assert!(
        !cargo.contains("[build-dependencies]"),
        "unexpected [build-dependencies] table:\n{cargo}"
    );
    insta::assert_snapshot!(project_to_md(&project));
}

/// Removing a section whose deps were the only owners of
/// `[build-dependencies]` (and similarly for `[features]` /
/// `[dependencies]`) must drop the now-empty tables, not leave headers
/// behind.
#[test]
fn remove_last_section_drops_empty_blocks() {
    let project = TestProject::new();
    {
        let mut scaffold = project.scaffold();
        scaffold
            .set_napi(NapiSectionOpts {
                content: "// napi\n".into(),
            })
            .unwrap();
        scaffold.deploy().unwrap();
    }
    {
        let mut scaffold = project.scaffold();
        scaffold.deploy().unwrap();
    }
    let cargo = fs::read_to_string(project.abs_root_path.join("Cargo.toml")).unwrap();
    assert!(
        !cargo.contains("[features]"),
        "unexpected [features]:\n{cargo}"
    );
    assert!(
        !cargo.contains("[dependencies]"),
        "unexpected [dependencies]:\n{cargo}"
    );
    assert!(
        !cargo.contains("[build-dependencies]"),
        "unexpected [build-dependencies]:\n{cargo}"
    );
    insta::assert_snapshot!(project_to_md(&project));
}

/// Removing only the section that contributed `[build-dependencies]`
/// should drop that table but keep `[features]` and `[dependencies]`
/// for the surviving section.
#[test]
fn remove_napi_drops_only_build_dependencies() {
    let project = TestProject::new();
    {
        let mut scaffold = project.scaffold();
        scaffold
            .set_napi(NapiSectionOpts {
                content: "// napi\n".into(),
            })
            .unwrap();
        scaffold
            .set_wasm(WasmSectionOpts {
                content: "// wasm\n".into(),
            })
            .unwrap();
        scaffold.deploy().unwrap();
    }
    {
        let mut scaffold = project.scaffold();
        scaffold
            .set_wasm(WasmSectionOpts {
                content: "// wasm\n".into(),
            })
            .unwrap();
        scaffold.deploy().unwrap();
    }
    let cargo = fs::read_to_string(project.abs_root_path.join("Cargo.toml")).unwrap();
    assert!(
        !cargo.contains("[build-dependencies]"),
        "unexpected [build-dependencies]:\n{cargo}"
    );
    assert!(cargo.contains("[features]"), "missing [features]:\n{cargo}");
    assert!(
        cargo.contains("[dependencies]"),
        "missing [dependencies]:\n{cargo}"
    );
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn workspace_inherited_napi_dep_preserved() {
    let project = TestProject::with_cargo_toml(WORKSPACE_CARGO_TOML);
    let mut scaffold = project.scaffold();
    scaffold
        .set_napi(NapiSectionOpts {
            content: "// napi\n".into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn deploy_is_byte_identical_on_rerun() {
    let project = TestProject::new();
    let snap = |project: &TestProject| -> String { project_to_md(project) };

    let (ir, exposed) = empty_ir(TEST_CRATE);
    let run = |project: &TestProject| {
        let mut scaffold = project.scaffold();
        scaffold
            .set_link(LinkSectionOpts {
                modules: &[],
                ir: &ir,
                exposed: &exposed,
            })
            .unwrap();
        scaffold
            .set_napi(NapiSectionOpts {
                content: "// n\n".into(),
            })
            .unwrap();
        scaffold.deploy().unwrap();
    };

    run(&project);
    let after_first = snap(&project);
    run(&project);
    let after_second = snap(&project);

    assert_eq!(
        after_first, after_second,
        "second deploy diverged from first"
    );

    let meta = fs::read_to_string(project.abs_temp_path.join("meta.json")).unwrap();
    let _: serde_json::Value = serde_json::from_str(&meta).unwrap();
}

#[test]
fn external_files_swap_removes_stale_paths() {
    let project = TestProject::new();
    {
        let mut scaffold = project.scaffold();
        scaffold
            .set(
                ScaffoldSection::new("napi")
                    .cargo_feature_flag("node")
                    .content("// n\n")
                    .external("napi/")
                    .external("side_artifact.txt"),
            )
            .unwrap();
        scaffold.deploy().unwrap();
    }

    let napi_dir = project.abs_temp_path.join("napi");
    fs::create_dir_all(&napi_dir).unwrap();
    fs::write(napi_dir.join("compiled.node"), b"binary").unwrap();
    fs::write(project.abs_temp_path.join("side_artifact.txt"), "hi").unwrap();
    fs::write(project.abs_temp_path.join("user_owned.txt"), "keep me").unwrap();

    {
        let mut scaffold = project.scaffold();
        scaffold
            .set(
                ScaffoldSection::new("napi")
                    .cargo_feature_flag("node")
                    .content("// n\n")
                    .external("napi/"),
            )
            .unwrap();
        scaffold.deploy().unwrap();
    }

    assert!(
        !project.abs_temp_path.join("side_artifact.txt").exists(),
        "stale external file should be deleted",
    );
    assert!(
        project.abs_temp_path.join("user_owned.txt").exists(),
        "non-owned file must be preserved",
    );
    assert!(napi_dir.exists(), "still-owned dir is preserved");
}

#[test]
fn remove_section_clears_its_external_files() {
    let project = TestProject::new();
    {
        let mut scaffold = project.scaffold();
        scaffold
            .set(
                ScaffoldSection::new("wasm")
                    .cargo_feature_flag("wasm")
                    .content("// w\n")
                    .external("wasm/"),
            )
            .unwrap();
        scaffold.deploy().unwrap();
    }
    let wasm_dir = project.abs_temp_path.join("wasm");
    fs::create_dir_all(&wasm_dir).unwrap();
    fs::write(wasm_dir.join("pkg.wasm"), b"binary").unwrap();

    {
        let mut scaffold = project.scaffold();
        scaffold.deploy().unwrap();
    }

    assert!(
        !wasm_dir.exists(),
        "removed section's external files cleaned up"
    );
}

#[test]
fn external_files_rejects_parent_traversal() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let err = scaffold
        .set(
            ScaffoldSection::new("napi")
                .cargo_feature_flag("node")
                .external("../escape"),
        )
        .unwrap_err();
    assert!(matches!(err, ScaffoldError::PathEscape(_)));
}

#[test]
fn build_contribution_renders_setup_fn() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set(
            ScaffoldSection::new("napi")
                .cargo_feature_flag("node")
                .content("// n\n")
                .build_content(BuildContribution {
                    preamble: "use napi_build;".into(),
                    main: "napi_build::setup();".into(),
                }),
        )
        .unwrap();
    scaffold.deploy().unwrap();

    let build_napi = fs::read_to_string(project.abs_temp_path.join("build-napi.rs")).unwrap();
    assert!(build_napi.contains("use napi_build;"));
    assert!(build_napi.contains("pub fn setup() {"));
    assert!(build_napi.contains("napi_build::setup();"));
}
