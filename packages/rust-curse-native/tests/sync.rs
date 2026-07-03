use std::{collections::BTreeMap, fs};

use rust_curse::{
    codegen::{
        bindings::{Api, ApiKind, WasmTarget},
        db::DbImpl,
        rustdoc::{Crate as StoredCrate, RustdocCrates},
    },
    sync::{
        cursed_crate::{sync_crate, SyncCrateOps},
        ts_tree::{sync_ts_tree, SyncTsTreeOpts},
    },
    tools,
};
use tempfile::{tempdir, TempDir};

#[test]
fn syncs_wasm_crate() {
    let fixture = fixture("sync-crate-wasm", None);
    let out_dir = fixture.dir.path().join(".rust-curse/crates/wasm");
    let db = empty_db(wasm_api());

    let changed = sync_crate(SyncCrateOps {
        manifest_path: &fixture.manifest_path,
        out_dir: &out_dir,
        db: &db,
    })
    .expect("sync wasm crate");

    assert!(changed);
    assert!(out_dir.join("Cargo.toml").is_file());
    assert!(out_dir.join("src/lib.rs").is_file());
    assert!(!out_dir.join("build.rs").exists());

    let manifest = fs::read_to_string(out_dir.join("Cargo.toml")).unwrap();
    assert!(manifest.contains("name = \"cursed-sync-crate-wasm\""));
    assert!(manifest.contains("wasm-bindgen = \"0.2\""));
    assert!(manifest.contains(&format!(
        "\"sync_crate_wasm\" = {{ package = \"sync-crate-wasm\", path = \"{}\" }}",
        fixture.dir.path().canonicalize().unwrap().display()
    )));

    let lib = fs::read_to_string(out_dir.join("src/lib.rs")).unwrap();
    assert!(lib.contains("use wasm_bindgen::prelude::*;"));

    let changed = sync_crate(SyncCrateOps {
        manifest_path: &fixture.manifest_path,
        out_dir: &out_dir,
        db: &db,
    })
    .expect("sync unchanged wasm crate");

    assert!(!changed);
}

#[test]
fn syncs_node_crate() {
    let fixture = fixture("sync-crate-node", Some("sync_crate_node_bindings"));
    let out_dir = fixture.dir.path().join(".rust-curse/crates/node");
    let db = empty_db(ApiKind::Node);

    let changed = sync_crate(SyncCrateOps {
        manifest_path: &fixture.manifest_path,
        out_dir: &out_dir,
        db: &db,
    })
    .expect("sync node crate");

    assert!(changed);
    assert!(out_dir.join("Cargo.toml").is_file());
    assert!(out_dir.join("src/lib.rs").is_file());
    assert!(out_dir.join("build.rs").is_file());

    let manifest = fs::read_to_string(out_dir.join("Cargo.toml")).unwrap();
    assert!(manifest.contains("name = \"cursed-sync-crate-node\""));
    assert!(manifest.contains("build = \"build.rs\""));
    assert!(manifest.contains("napi = { version = \"3\", features = [\"napi10\"] }"));
    assert!(manifest.contains("napi-build = \"2\""));
    assert!(manifest.contains(&format!(
        "\"sync_crate_node_bindings\" = {{ package = \"sync-crate-node\", path = \"{}\" }}",
        fixture.dir.path().canonicalize().unwrap().display()
    )));

    let lib = fs::read_to_string(out_dir.join("src/lib.rs")).unwrap();
    assert!(lib.contains("use napi_derive::napi;"));

    let build_script = fs::read_to_string(out_dir.join("build.rs")).unwrap();
    assert!(build_script.contains("fn main()"));
    assert!(build_script.contains("napi_build::setup();"));

    let changed = sync_crate(SyncCrateOps {
        manifest_path: &fixture.manifest_path,
        out_dir: &out_dir,
        db: &db,
    })
    .expect("sync unchanged node crate");

    assert!(!changed);
}

#[test]
fn syncs_ts_tree_sourcemaps() {
    let fixture = fixture("sync_ts_tree", None);
    let rustdoc = tools::rustdoc::run(&fixture.manifest_path).expect("generate rustdoc");
    let mut db = DbImpl::default();
    RustdocCrates::set(
        &mut db,
        BTreeMap::from([(
            "sync_ts_tree".to_string(),
            StoredCrate {
                body: rustdoc,
                is_external: false,
                dir: fixture.dir.path().to_path_buf(),
            },
        )]),
    );
    let out_dir = fixture.dir.path().join(".rust-curse/ts");
    let src_dir = fixture.dir.path().join("src");

    sync_ts_tree(SyncTsTreeOpts {
        our_dir: &out_dir,
        src_dir: &src_dir,
        db: &db,
    })
    .expect("sync TypeScript tree");

    let declarations_path = out_dir.join("src/lib.rs.d.ts");
    let sourcemap_path = out_dir.join("src/lib.rs.d.ts.map");
    assert!(declarations_path.is_file());
    assert!(sourcemap_path.is_file());

    let declarations = fs::read_to_string(declarations_path).unwrap();
    assert!(declarations.contains("export function marker(): void;"));
    assert!(declarations.contains("//# sourceMappingURL=lib.rs.d.ts.map"));

    let sourcemap = fs::read_to_string(sourcemap_path).unwrap();
    let source_path = fixture
        .dir
        .path()
        .join("src/lib.rs")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    assert!(sourcemap.contains(&format!(r#""sources":["{source_path}"]"#)));
    assert!(sourcemap.contains(r#""names":["marker"]"#));
    assert!(sourcemap.contains(r#""mappings":"AAAAA,gBAAOA""#));
}

struct Fixture {
    dir: TempDir,
    manifest_path: std::path::PathBuf,
}

fn fixture(package_name: &str, lib_name: Option<&str>) -> Fixture {
    let dir = tempdir().unwrap();
    let manifest_path = dir.path().join("Cargo.toml");
    let lib_section = lib_name
        .map(|name| format!("\n[lib]\nname = \"{name}\"\n"))
        .unwrap_or_default();
    let manifest = format!(
        "[package]\n\
         name = \"{package_name}\"\n\
         version = \"1.2.3\"\n\
         edition = \"2021\"\n\
         publish = false\n\
         {lib_section}"
    );

    fs::write(&manifest_path, manifest).unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/lib.rs"), "pub fn marker() {}\n").unwrap();

    Fixture { dir, manifest_path }
}

fn wasm_api() -> ApiKind {
    ApiKind::Wasm {
        target: WasmTarget::Web,
    }
}

fn empty_db(api: ApiKind) -> DbImpl {
    let mut db = DbImpl::default();
    Api::set(&mut db, api);
    RustdocCrates::set(&mut db, BTreeMap::<String, StoredCrate>::new());
    db
}
