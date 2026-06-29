//! Utilities for building insta snapshots from rustdoc fixtures.

use crate::utils::{diags, ir_debug::DebugRender as _, rustdoc};
use rust_curse::codegen::{
    bindings::{self, Api, WasmTarget},
    db::Db,
    diag::Theme,
    js,
    rustdoc::{all_mod_ids, lower_mod, Id},
    ts,
};
use rustdoc_types::{Crate as RustdocCrate, Id as RId};

/// Builds a snapshot of the given source code, rendering the requested
/// categories.
pub fn build_snap(raw_src: &str, categories: &[&str]) -> String {
    let fixture = rustdoc::rustdoc_from_src(raw_src);
    let db = rustdoc::db_with_rustdoc(fixture.crate_.clone(), fixture.crate_dir.clone());
    let root_id = rustdoc::root_id(&db, &fixture.crate_);
    let module_ids = all_mod_ids(&db);

    let mut body = String::new();
    let thread = std::thread::current();
    let test_name = thread.name().unwrap_or("<unknown>");

    push_section(&mut body, "Test", "text", test_name);
    push_section(&mut body, "Source", "rs", &fixture.src);

    for category in categories {
        match *category {
            "diags" => {
                let text = build_diags_body(&db, root_id);
                push_section(&mut body, "Diagnostics", "text", &text);
            }
            "ir" => {
                let root_mod = lower_mod(&db, root_id);
                let text = root_mod.render(&db);
                let text = if text.trim().is_empty() {
                    "None\n"
                } else {
                    text.as_str()
                };
                push_section(&mut body, "IR", "rs", text);
            }
            "ts" => {
                let text = build_typescript_body(&db, &fixture.crate_, module_ids);
                push_markdown_section(&mut body, "Typescript", &text);
            }
            "js" => {
                let text = build_javascript_body(&db, &fixture.crate_, module_ids, Api::Node);
                push_markdown_section(&mut body, "Javascript", &text);
            }
            "wasm_web_js" => {
                let text = build_javascript_body(&db, &fixture.crate_, module_ids, wasm_web_api());
                push_markdown_section(&mut body, "Wasm Web Javascript", &text);
            }
            "wasm" => {
                let text = bindings::compile(&db, wasm_web_api());
                push_section(&mut body, "Wasm Bindings", "rs", &text);
            }
            "node" => {
                let text = bindings::compile(&db, Api::Node);
                push_section(&mut body, "Node Bindings", "rs", &text);
            }
            other => panic!("unknown snapshot category `{other}`"),
        }
    }

    body
}

fn wasm_web_api() -> Api {
    Api::Wasm {
        target: WasmTarget::Web,
    }
}

fn build_diags_body(db: &dyn Db, root_id: Id<'_>) -> String {
    let diags = diags::collect_diags_mod(db, root_id);

    if diags.is_empty() {
        "None\n".to_string()
    } else {
        diags
            .into_iter()
            .map(|diag| {
                diag.render(Theme::UnicodeNoColor)
                    .expect("render diagnostic")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn build_typescript_body(db: &dyn Db, crate_: &RustdocCrate, module_ids: &[Id<'_>]) -> String {
    let mut text = String::new();

    for id in module_ids {
        let ts = ts::compile_mod(db, *id);
        push_module_code_block(&mut text, db, crate_, *id, "ts", ts);
    }

    if text.trim().is_empty() {
        "None\n".to_string()
    } else {
        text
    }
}

fn build_javascript_body(
    db: &dyn Db,
    crate_: &RustdocCrate,
    module_ids: &[Id<'_>],
    api: Api,
) -> String {
    let mut text = String::new();

    for id in module_ids {
        let js = js::compile_mod(db, api, *id, "./bindings.js".to_string());
        push_module_code_block(&mut text, db, crate_, *id, "js", js);
    }

    if text.trim().is_empty() {
        "None\n".to_string()
    } else {
        text
    }
}

fn push_module_code_block(
    text: &mut String,
    db: &dyn Db,
    crate_: &RustdocCrate,
    id: Id<'_>,
    lang: &str,
    code: &str,
) {
    if code.trim().is_empty() {
        return;
    }

    if !text.is_empty() {
        text.push('\n');
    }

    let module_name = module_path_display(crate_, id.idx(db));
    text.push_str(&format!("for `{module_name}`:\n\n"));
    text.push_str("```");
    text.push_str(lang);
    text.push('\n');
    text.push_str(code);
    if !code.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("```\n");
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

fn module_path_display(crate_: &RustdocCrate, id: RId) -> String {
    crate_
        .paths
        .get(&id)
        .map(|summary| summary.path.join("::"))
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "crate".to_string())
}
