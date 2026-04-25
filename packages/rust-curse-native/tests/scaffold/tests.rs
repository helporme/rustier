use std::fs;

use rust_curse::scaffold::default::DefaultSectionOpts;
use rust_curse::scaffold::napi::NapiSectionOpts;
use rust_curse::scaffold::rustdoc::RustdocSectionOpts;
use rust_curse::scaffold::wasm::WasmSectionOpts;
use rust_curse::scaffold::{BuildContribution, ScaffoldError, ScaffoldSection};

use crate::common::{project_to_md, TestProject, WORKSPACE_CARGO_TOML};

#[test]
fn set_default_only() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set_default(DefaultSectionOpts {
            lib_content: "// default lib body\n".into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn set_napi_only() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set_napi(NapiSectionOpts {
            feature_name: "node".into(),
            lib_content: "// napi glue\n".into(),
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
            lib_content: "// wasm glue\n".into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn set_rustdoc_only() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set_rustdoc(RustdocSectionOpts {
            lib_content: "pub mod foo;\n".into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn set_combo_all_four_sections() {
    let project = TestProject::new();
    let mut scaffold = project.scaffold();
    scaffold
        .set_default(DefaultSectionOpts {
            lib_content: "// default\n".into(),
        })
        .unwrap();
    scaffold
        .set_napi(NapiSectionOpts {
            feature_name: "node".into(),
            lib_content: "// napi\n".into(),
        })
        .unwrap();
    scaffold
        .set_wasm(WasmSectionOpts {
            feature_name: "wasm".into(),
            lib_content: "// wasm\n".into(),
        })
        .unwrap();
    scaffold
        .set_rustdoc(RustdocSectionOpts {
            lib_content: "// rustdoc\n".into(),
        })
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
            lib_content: "// original\n".into(),
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
                lib_content: "// napi\n".into(),
            })
            .unwrap();
        scaffold
            .set_wasm(WasmSectionOpts {
                feature_name: "wasm".into(),
                lib_content: "// wasm\n".into(),
            })
            .unwrap();
        scaffold.deploy().unwrap();
    }
    {
        let mut scaffold = project.scaffold();
        scaffold
            .set_wasm(WasmSectionOpts {
                feature_name: "wasm".into(),
                lib_content: "// wasm\n".into(),
            })
            .unwrap();
        scaffold.deploy().unwrap();
    }
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn workspace_inherited_napi_dep_preserved() {
    let project = TestProject::with_cargo_toml(WORKSPACE_CARGO_TOML);
    let mut scaffold = project.scaffold();
    scaffold
        .set_napi(NapiSectionOpts {
            feature_name: "node".into(),
            lib_content: "// napi\n".into(),
        })
        .unwrap();
    scaffold.deploy().unwrap();
    insta::assert_snapshot!(project_to_md(&project));
}

#[test]
fn deploy_is_byte_identical_on_rerun() {
    let project = TestProject::new();
    let snap = |project: &TestProject| -> String { project_to_md(project) };

    let run = |project: &TestProject| {
        let mut scaffold = project.scaffold();
        scaffold
            .set_default(DefaultSectionOpts {
                lib_content: "// d\n".into(),
            })
            .unwrap();
        scaffold
            .set_napi(NapiSectionOpts {
                feature_name: "node".into(),
                lib_content: "// n\n".into(),
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
