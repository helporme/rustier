use std::fs;
use std::path::PathBuf;

use rust_curse::orphans::{OrphanModule, OrphanModuleMembers, OrphanModuleShaked};
use rust_curse::scaffold::default::DefaultSectionOpts;
use rust_curse::scaffold::napi::NapiSectionOpts;
use rust_curse::scaffold::rustdoc::RustdocSectionOpts;
use rust_curse::scaffold::ts::{TsSectionOpts, TS_FILE};
use rust_curse::scaffold::wasm::WasmSectionOpts;
use rust_curse::scaffold::{BuildContribution, ScaffoldError, ScaffoldSection};
use rustdoc_types::Visibility;

use crate::common::{make_crate, make_item, project_to_md, TestProject, WORKSPACE_CARGO_TOML};

fn test_module(mod_name: &str, rel_rs_path: &str) -> OrphanModule {
    OrphanModule {
        rel_rs_path: PathBuf::from(rel_rs_path),
        mod_name: mod_name.to_string(),
    }
}

fn test_shaked(
    mod_name: &str,
    rel_rs_path: &str,
    members: OrphanModuleMembers,
) -> OrphanModuleShaked {
    OrphanModuleShaked {
        rel_rs_path: PathBuf::from(rel_rs_path),
        mod_name: mod_name.to_string(),
        imported_members: members,
    }
}

fn empty_default_opts<'a>(crate_: &'a rustdoc_types::Crate) -> DefaultSectionOpts<'a> {
    DefaultSectionOpts {
        modules: &[],
        crate_,
    }
}

#[test]
fn set_default_only() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let crate_ = make_crate(vec![]);
    scaffold.set_default(empty_default_opts(&crate_)).unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn default_emits_mod_decls_and_treeshaked_reexports() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

    let modules = vec![
        test_shaked(
            "greet",
            "src/greet.rs",
            OrphanModuleMembers::Specific(vec!["hello".into()]),
        ),
        test_shaked("unused", "src/unused.rs", OrphanModuleMembers::None),
    ];
    let crate_ = make_crate(vec![
        make_item(1, "hello", "src/greet.rs", Visibility::Public),
        make_item(2, "unused_fn", "src/unused.rs", Visibility::Public),
    ]);

    scaffold
        .set_default(DefaultSectionOpts {
            modules: &modules,
            crate_: &crate_,
        })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn default_mangled_names_get_allow_attribute() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

    let modules = vec![test_shaked(
        "nested::deep",
        "src/nested/deep.rs",
        OrphanModuleMembers::Specific(vec!["run".into()]),
    )];
    let crate_ = make_crate(vec![make_item(
        1,
        "run",
        "src/nested/deep.rs",
        Visibility::Public,
    )]);

    scaffold
        .set_default(DefaultSectionOpts {
            modules: &modules,
            crate_: &crate_,
        })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn default_no_reexports_when_nothing_imported() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

    let modules = vec![test_shaked(
        "greet",
        "src/greet.rs",
        OrphanModuleMembers::None,
    )];
    let crate_ = make_crate(vec![make_item(
        1,
        "hello",
        "src/greet.rs",
        Visibility::Public,
    )]);

    scaffold
        .set_default(DefaultSectionOpts {
            modules: &modules,
            crate_: &crate_,
        })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn default_star_import_re_exports_public_only() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

    let modules = vec![test_shaked("math", "src/math.rs", OrphanModuleMembers::All)];
    let crate_ = make_crate(vec![
        make_item(1, "add", "src/math.rs", Visibility::Public),
        make_item(2, "sub", "src/math.rs", Visibility::Public),
        make_item(3, "internal", "src/math.rs", Visibility::Default),
    ]);

    scaffold
        .set_default(DefaultSectionOpts {
            modules: &modules,
            crate_: &crate_,
        })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

/// Default with real modules + napi: lib.rs default include must be
/// gated by `not(any(feature = "node"))`.
#[test]
fn default_with_modules_combined_with_napi() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

    let modules = vec![test_shaked(
        "greet",
        "src/greet.rs",
        OrphanModuleMembers::Specific(vec!["hello".into()]),
    )];
    let crate_ = make_crate(vec![make_item(
        1,
        "hello",
        "src/greet.rs",
        Visibility::Public,
    )]);

    scaffold
        .set_default(DefaultSectionOpts {
            modules: &modules,
            crate_: &crate_,
        })
        .unwrap();
    scaffold
        .set_napi(NapiSectionOpts {
            feature_name: "node".into(),
            content: "// napi glue\n".into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

/// Default with real modules + rustdoc (env-activated): lib.rs default
/// include must be gated by `not(any(rust_curse_rustdoc))`, and rustdoc
/// gets `#[cfg(rust_curse_rustdoc)]`.
#[test]
fn default_with_modules_combined_with_rustdoc() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

    let modules = vec![test_shaked(
        "greet",
        "src/greet.rs",
        OrphanModuleMembers::Specific(vec!["hello".into()]),
    )];
    let crate_ = make_crate(vec![make_item(
        1,
        "hello",
        "src/greet.rs",
        Visibility::Public,
    )]);

    scaffold
        .set_default(DefaultSectionOpts {
            modules: &modules,
            crate_: &crate_,
        })
        .unwrap();
    scaffold
        .set_rustdoc(RustdocSectionOpts {
            modules: &[test_module("greet", "greet.rs")],
        })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

/// Default with real modules + napi + wasm: lib.rs default include must
/// be gated by `not(any(feature = "node", feature = "wasm"))`, exercising
/// the multi-exclusion path of the dispatcher.
#[test]
fn default_with_modules_combined_with_napi_and_wasm() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();

    let modules = vec![test_shaked("math", "src/math.rs", OrphanModuleMembers::All)];
    let crate_ = make_crate(vec![
        make_item(1, "add", "src/math.rs", Visibility::Public),
        make_item(2, "sub", "src/math.rs", Visibility::Public),
    ]);

    scaffold
        .set_default(DefaultSectionOpts {
            modules: &modules,
            crate_: &crate_,
        })
        .unwrap();
    scaffold
        .set_napi(NapiSectionOpts {
            feature_name: "node".into(),
            content: "// napi glue\n".into(),
        })
        .unwrap();
    scaffold
        .set_wasm(WasmSectionOpts {
            feature_name: "wasm".into(),
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
            feature_name: "node".into(),
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
            feature_name: "wasm".into(),
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
    scaffold
        .set_ts(TsSectionOpts {
            content: "export declare function hello(): void;\n".into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn set_ts_writes_ambient_dts() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let content = "export declare const VERSION: string;\n";
    scaffold
        .set_ts(TsSectionOpts {
            content: content.into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    let written = fs::read_to_string(project.abs_temp_path.join(TS_FILE)).unwrap();
    assert_eq!(written, content);
}

#[test]
fn set_ts_combined_with_napi() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set_napi(NapiSectionOpts {
            feature_name: "node".into(),
            content: "// napi glue\n".into(),
        })
        .unwrap();
    scaffold
        .set_ts(TsSectionOpts {
            content: "export declare function hello(): void;\n".into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn remove_ts_clears_ambient_dts() {
    let project = TestProject::new();
    {
        let mut scaffold = project.scaffold();
        scaffold
            .set_ts(TsSectionOpts {
                content: "export declare function hello(): void;\n".into(),
            })
            .unwrap();
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
fn rustdoc_mangles_nested_module_names() {
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
fn rustdoc_underscored_names_dont_collide() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let modules = vec![
        test_module("nested::file", "nested/file.rs"),
        test_module("nested__::file", "nested__/file.rs"),
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
fn rustdoc_underscored_directory_names() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let modules = vec![test_module("__nested::__deep", "__nested/__deep.rs")];
    scaffold
        .set_rustdoc(RustdocSectionOpts { modules: &modules })
        .unwrap();
    scaffold.deploy().unwrap();
    project.snapshot_settings().bind(|| {
        insta::assert_snapshot!(project_to_md(&project));
    });
}

#[test]
fn set_combo_all_four_sections() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let crate_ = make_crate(vec![]);
    scaffold.set_default(empty_default_opts(&crate_)).unwrap();
    scaffold
        .set_napi(NapiSectionOpts {
            feature_name: "node".into(),
            content: "// napi\n".into(),
        })
        .unwrap();
    scaffold
        .set_wasm(WasmSectionOpts {
            feature_name: "wasm".into(),
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
fn patch_replaces_only_lib_content() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set_napi(NapiSectionOpts {
            feature_name: "node".into(),
            content: "// original\n".into(),
        })
        .unwrap();
    scaffold
        .patch(ScaffoldSection::new("napi").lib("// patched\n"))
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn patch_unknown_section_errors() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let err = scaffold
        .patch(ScaffoldSection::new("nope").lib("x"))
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
                feature_name: "node".into(),
                content: "// napi\n".into(),
            })
            .unwrap();
        scaffold
            .set_wasm(WasmSectionOpts {
                feature_name: "wasm".into(),
                content: "// wasm\n".into(),
            })
            .unwrap();
        scaffold.deploy().unwrap();
    }
    {
        let mut scaffold = project.scaffold();
        scaffold
            .set_wasm(WasmSectionOpts {
                feature_name: "wasm".into(),
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
fn default_only_emits_no_empty_blocks() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    let crate_ = make_crate(vec![]);
    scaffold.set_default(empty_default_opts(&crate_)).unwrap();
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
                feature_name: "node".into(),
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
                feature_name: "node".into(),
                content: "// napi\n".into(),
            })
            .unwrap();
        scaffold
            .set_wasm(WasmSectionOpts {
                feature_name: "wasm".into(),
                content: "// wasm\n".into(),
            })
            .unwrap();
        scaffold.deploy().unwrap();
    }
    {
        let mut scaffold = project.scaffold();
        scaffold
            .set_wasm(WasmSectionOpts {
                feature_name: "wasm".into(),
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
            feature_name: "node".into(),
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

    let crate_ = make_crate(vec![]);
    let run = |project: &TestProject| {
        let mut scaffold = project.scaffold();
        scaffold.set_default(empty_default_opts(&crate_)).unwrap();
        scaffold
            .set_napi(NapiSectionOpts {
                feature_name: "node".into(),
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
                    .lib("// n\n")
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
                    .lib("// n\n")
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
                    .lib("// w\n")
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
                .lib("// n\n")
                .build(BuildContribution {
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
