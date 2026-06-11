//! Utilities for generating rustdoc and derivatives from it.

#![allow(dead_code, unused)]

use std::{collections::BTreeMap, fs, path::Path, process::Command};

use crate::utils::ir_debug::DebugRender as _;
use rust_curse::{
    codegen::{
        db::{Db, DbImpl},
        ir::Mod,
        rustdoc::{lower_mod, lower_mod_fns, RustdocId, RustdocInput},
        ts,
    },
    diag::{render_diags, Diag, DiagRendered, Theme},
};
use rustdoc_types::{Crate as RustdocCrate, Id, ItemEnum};
use tempfile::TempDir;

const TEST_CRATE_NAME: &str = "rustdoc_test_crate";

pub struct Fixture {
    pub _dir: TempDir,
    pub root_abs_path: std::path::PathBuf,
    pub src: String,
    pub crate_: RustdocCrate,
}

pub fn rustdoc_from_src(src: &str) -> Fixture {
    let src = fmt_src(src);
    let dir = tempfile::tempdir().expect("create tempdir");
    let root_abs_path = dir.path().to_path_buf();

    let cargo_toml = format!(
        "[package]\n\
         name = \"{TEST_CRATE_NAME}\"\n\
         version = \"0.0.0\"\n\
         edition = \"2021\"\n\
         publish = false\n\
         \n\
         [lib]\n\
         path = \"src/lib.rs\"\n\
         \n\
         [workspace]\n"
    );

    fs::write(root_abs_path.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
    fs::create_dir_all(root_abs_path.join("src")).expect("create src/");
    fs::write(root_abs_path.join("src/lib.rs"), &src).expect("write src/lib.rs");

    let target_abs_path = root_abs_path.join("target");
    let crate_ = run_rustdoc(&root_abs_path, &target_abs_path, TEST_CRATE_NAME)
        .unwrap_or_else(|err| panic!("rustdoc generation failed: {err}"));

    Fixture {
        _dir: dir,
        root_abs_path,
        src,
        crate_,
    }
}

pub fn fmt_src(src: &str) -> String {
    match syn::parse_str::<syn::File>(src) {
        Ok(file) => prettyplease::unparse(&file),
        Err(_) => src.to_string(),
    }
}

fn run_rustdoc(
    project_abs_path: &Path,
    target_abs_path: &Path,
    lib_name: &str,
) -> Result<RustdocCrate, String> {
    let output = Command::new("cargo")
        .args([
            "+nightly",
            "rustdoc",
            "--lib",
            "--",
            "-Z",
            "unstable-options",
            "--output-format",
            "json",
        ])
        .env("RUST_CURSE_RUSTDOC", "1")
        .env("CARGO_TARGET_DIR", target_abs_path)
        .current_dir(project_abs_path)
        .output()
        .map_err(|err| format!("cargo is not available: {err}"))?;

    if !output.status.success() {
        return Err(format!(
            "cargo rustdoc failed (exit code {:?}):\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let json_abs_path = target_abs_path.join("doc").join(format!("{lib_name}.json"));
    let json = fs::read_to_string(&json_abs_path)
        .map_err(|err| format!("failed to read '{}': {err}", json_abs_path.display()))?;

    serde_json::from_str(&json).map_err(|err| format!("failed to parse rustdoc JSON: {err}"))
}

pub fn db_with_rustdoc(crate_: RustdocCrate) -> DbImpl {
    let db = DbImpl::default();
    let crates = BTreeMap::from([(TEST_CRATE_NAME.to_string(), crate_)]);
    RustdocInput::new(&db, crates);
    db
}

pub fn root_mod<'db>(db: &'db dyn Db, crate_: &RustdocCrate) -> Mod<'db> {
    lower_mod(db, RustdocId::new(db, crate_.root, TEST_CRATE_NAME))
}

pub fn lower_root_fns_and_diags(db: &dyn Db, crate_: &RustdocCrate) -> Vec<Diag> {
    let root_id = RustdocId::new(db, crate_.root, TEST_CRATE_NAME);
    let _ = lower_mod_fns(db, root_id);
    lower_mod_fns::accumulated::<Diag>(db, root_id)
        .into_iter()
        .cloned()
        .collect()
}

fn mod_ids(crate_: &RustdocCrate) -> Vec<Id> {
    let mut ids = Vec::new();
    collect_mod_ids(crate_, crate_.root, &mut ids);
    ids
}

fn collect_mod_ids(crate_: &RustdocCrate, id: Id, ids: &mut Vec<Id>) {
    let item = crate_.index.get(&id).expect("module item not found");
    let ItemEnum::Module(module) = &item.inner else {
        return;
    };

    ids.push(id);

    for child_id in &module.items {
        let Some(child) = crate_.index.get(child_id) else {
            continue;
        };

        if matches!(child.inner, ItemEnum::Module(_)) {
            collect_mod_ids(crate_, *child_id, ids);
        }
    }
}

pub fn rendered_diags(diags: Vec<Diag>, root_abs_path: &Path) -> Vec<DiagRendered> {
    render_diags(diags, root_abs_path, Theme::UnicodeNoColor).expect("render diagnostics")
}

pub fn build_snap(raw_src: &str, categories: &[&str]) -> String {
    let fixture = rustdoc_from_src(raw_src);
    let db = db_with_rustdoc(fixture.crate_.clone());
    let module_ids = mod_ids(&fixture.crate_);
    let mods = module_ids
        .iter()
        .map(|id| lower_mod(&db, RustdocId::new(&db, *id, TEST_CRATE_NAME)))
        .collect::<Vec<_>>();
    let diags = lower_root_fns_and_diags(&db, &fixture.crate_);

    let mut body = String::new();
    let thread = std::thread::current();
    let test_name = thread.name().unwrap_or("<unknown>");
    push_section(&mut body, "Test", "text", test_name);
    push_section(&mut body, "Source", "rs", &fixture.src);

    for category in categories {
        match *category {
            "diags" | "diag" | "diagnostics" => {
                let rendered = rendered_diags(diags.clone(), &fixture.root_abs_path);
                let text = if rendered.is_empty() {
                    "None\n".to_string()
                } else {
                    rendered
                        .into_iter()
                        .filter_map(|diag| diag.rendered)
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                push_section(&mut body, "Diagnostics", "text", &text);
            }
            "ir" | "intermediate" => {
                let text = mods[0].render(&db);
                let text = if text.trim().is_empty() {
                    "None\n"
                } else {
                    text.as_str()
                };
                push_section(&mut body, "IR", "rs", text);
            }
            "ts" | "typescript" => {
                let mut text = String::new();

                for id in &module_ids {
                    let ts = ts::compile_mod(&db, RustdocId::new(&db, *id, TEST_CRATE_NAME));
                    if ts.trim().is_empty() {
                        continue;
                    }

                    if !text.is_empty() {
                        text.push('\n');
                    }

                    let module_name = module_path_display(&fixture.crate_, *id);
                    text.push_str(&format!("for `{module_name}`:\n\n"));
                    text.push_str("```ts\n");
                    text.push_str(ts);
                    if !ts.ends_with('\n') {
                        text.push('\n');
                    }
                    text.push_str("```\n");
                }

                let text = if text.trim().is_empty() {
                    "None\n"
                } else {
                    text.as_str()
                };
                push_markdown_section(&mut body, "Typescript", text);
            }
            other => panic!("unknown snapshot category `{other}`"),
        }
    }

    body
}

fn push_section(body: &mut String, name: &str, lang: &str, content: &str) {
    body.push_str(&format!("## {name}\n\n"));
    body.push_str("```");
    body.push_str(lang);
    body.push('\n');
    body.push_str(content);
    if !content.ends_with('\n') {
        body.push('\n');
    }
    body.push_str("```\n\n");
}

fn push_markdown_section(body: &mut String, name: &str, content: &str) {
    body.push_str(&format!("## {name}\n\n"));
    body.push_str(content);
    if !content.ends_with('\n') {
        body.push('\n');
    }
    body.push('\n');
}

fn module_path_display(crate_: &RustdocCrate, id: Id) -> String {
    crate_
        .paths
        .get(&id)
        .map(|summary| summary.path.join("::"))
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "crate".to_string())
}

macro_rules! rustdoc {
    ($($tt:tt)*) => {{
        rustdoc_from_src(stringify!($($tt)*)).crate_
    }};
}
pub(crate) use rustdoc;

macro_rules! ir {
    ($($tt:tt)*) => {{
        let crate_ = rustdoc! { $($tt)* };
        let db: &'static rust_curse::codegen::db::DbImpl =
            Box::leak(Box::new(db_with_rustdoc(crate_.clone())));
        let mod_ = root_mod(db, &crate_);
        (mod_, db)
    }};
}
pub(crate) use ir;

macro_rules! diag {
    ($($tt:tt)*) => {{
        let crate_ = rustdoc! { $($tt)* };
        let db: &'static rust_curse::codegen::db::DbImpl =
            Box::leak(Box::new(db_with_rustdoc(crate_.clone())));
        let diags = lower_root_fns_and_diags(db, &crate_);
        (diags, db)
    }};
}
pub(crate) use diag;

macro_rules! assert_snap {
    (@gen: [$($category:ident),+ $(,)?]; @snapshot_path: $snapshot_path:expr; $($tt:tt)*) => {{
        let categories = &[$(stringify!($category)),+];
        let body = build_snap(stringify!($($tt)*), categories);
        insta::with_settings!({ snapshot_path => $snapshot_path, prepend_module_to_snapshot => false }, {
            insta::assert_snapshot!(body);
        });
    }};
    (@gen: $($category:ident),+ $(,)?; @category: $snapshot_category:ident; $($tt:tt)*) => {{
        assert_snap!(
            @gen: [$($category),+];
            @snapshot_path: concat!("snapshots/", stringify!($snapshot_category));
            $($tt)*
        )
    }};
    (@gen: $($category:ident),+ $(,)?; $($tt:tt)*) => {{
        assert_snap!(
            @gen: [$($category),+];
            @snapshot_path: "snapshots";
            $($tt)*
        )
    }};
    ({ [$($category:ident),+ $(,)?] { $($tt:tt)* } }) => {
        assert_snap!(@gen: $($category),+; $($tt)*)
    };
    ([$($category:ident),+ $(,)?] { $($tt:tt)* }) => {
        assert_snap!(@gen: $($category),+; $($tt)*)
    };
}
pub(crate) use assert_snap;
