//! Utilities for building insta snapshots from rustdoc fixtures.

use crate::utils::{diags, ir_debug::DebugRender as _, rustdoc};
use rust_curse::{
    codegen::{
        db::Db,
        rustdoc::{lower_mod, RustdocId},
        ts,
    },
    diag::{render_diags, Theme},
};
use rustdoc_types::{Crate as RustdocCrate, Id, ItemEnum};

/// Builds a snapshot of the given source code, rendering the requested
/// categories.
pub fn build_snap(raw_src: &str, categories: &[&str]) -> String {
    let fixture = rustdoc::rustdoc_from_src(raw_src);
    let db = rustdoc::db_with_rustdoc(fixture.crate_.clone());
    let root_id = rustdoc::root_id(&db, &fixture.crate_);
    let module_ids = module_ids(&fixture.crate_);

    let mut body = String::new();
    let thread = std::thread::current();
    let test_name = thread.name().unwrap_or("<unknown>");

    push_section(&mut body, "Test", "text", test_name);
    push_section(&mut body, "Source", "rs", &fixture.src);

    for category in categories {
        match *category {
            "diags" => {
                let text = build_diags_body(&db, root_id, &fixture);
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
                let text = build_typescript_body(&db, &fixture.crate_, &module_ids);
                push_markdown_section(&mut body, "Typescript", &text);
            }
            other => panic!("unknown snapshot category `{other}`"),
        }
    }

    body
}

fn build_diags_body(db: &dyn Db, root_id: RustdocId<'_>, fixture: &rustdoc::Fixture) -> String {
    let diags = diags::collect_diags_mod(db, root_id);
    let rendered = render_diags(diags, &fixture.root_abs_path, Theme::UnicodeNoColor)
        .expect("render diagnostics");

    if rendered.is_empty() {
        "None\n".to_string()
    } else {
        rendered
            .into_iter()
            .filter_map(|diag| diag.rendered)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn build_typescript_body(db: &dyn Db, crate_: &RustdocCrate, module_ids: &[Id]) -> String {
    let mut text = String::new();

    for id in module_ids {
        let ts = ts::compile_mod(db, RustdocId::new(db, *id, rustdoc::TEST_CRATE_NAME));
        if ts.trim().is_empty() {
            continue;
        }

        if !text.is_empty() {
            text.push('\n');
        }

        let module_name = module_path_display(crate_, *id);
        text.push_str(&format!("for `{module_name}`:\n\n"));
        text.push_str("```ts\n");
        text.push_str(ts);
        if !ts.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("```\n");
    }

    if text.trim().is_empty() {
        "None\n".to_string()
    } else {
        text
    }
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

fn module_ids(crate_: &RustdocCrate) -> Vec<Id> {
    let mut ids = Vec::new();
    collect_module_ids(crate_, crate_.root, &mut ids);
    ids
}

fn collect_module_ids(crate_: &RustdocCrate, id: Id, ids: &mut Vec<Id>) {
    ids.push(id);

    let Some(item) = crate_.index.get(&id) else {
        return;
    };
    let ItemEnum::Module(module) = &item.inner else {
        return;
    };

    for child_id in &module.items {
        let Some(child) = crate_.index.get(child_id) else {
            continue;
        };
        if matches!(child.inner, ItemEnum::Module(_)) {
            collect_module_ids(crate_, *child_id, ids);
        }
    }
}

fn module_path_display(crate_: &RustdocCrate, id: Id) -> String {
    crate_
        .paths
        .get(&id)
        .map(|summary| summary.path.join("::"))
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "crate".to_string())
}
