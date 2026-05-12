//! Shared test helpers.

#![allow(dead_code)]

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use rust_curse::*;
use tempfile::TempDir;

const TEST_CRATE_NAME: &str = "rustdoc_test_crate";

/// Render a rustdoc crate from inline source. Returns the `TempDir` so
/// callers can keep source files alive across `IR::build` (warning
/// emission reads source for miette snippets).
pub fn gen_rustdoc(src: &str) -> (TempDir, PathBuf, rustdoc_types::Crate) {
    let project_dir = tempfile::tempdir().expect("create tempdir");
    let project_abs_path = project_dir.path().to_path_buf();

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
    fs::write(project_abs_path.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
    fs::create_dir_all(project_abs_path.join("src")).expect("create src/");
    fs::write(project_abs_path.join("src/lib.rs"), src).expect("write src/lib.rs");

    let target_abs_dir = project_abs_path.join("target");

    let crate_ = rustdoc::run(&project_abs_path, &target_abs_dir, TEST_CRATE_NAME)
        .unwrap_or_else(|e| panic!("rustdoc generation failed: {e}"));

    (project_dir, project_abs_path, crate_)
}

pub fn fmt_src(src: &str) -> String {
    match syn::parse_str::<syn::File>(src) {
        Ok(file) => prettyplease::unparse(&file),
        Err(_) => src.to_string(),
    }
}

/// Print the ambient preamble once, then every module's `.rs.d.ts` body
/// under a `// <rel_dts_path>` header. Single snapshot per test. The
/// crate root maps to `<rel_src_path>/lib.rs.d.ts`.
pub fn print_ts_per_mod(ir: &rust_curse::ir::IR, rel_src_path: std::path::PathBuf) -> String {
    let mut out = ir.print_ts_ambient().to_string();
    for (i, item) in ir.items.iter().enumerate() {
        if !matches!(item, rust_curse::ir::Item::Mod(_)) {
            continue;
        }
        let mid = rust_curse::ir::Id(i as u32);
        let segs = &ir.get_mod_unchecked(mid).path.segments;
        if segs.is_empty() {
            continue;
        }
        let mut dts = rel_src_path.clone();
        if segs.len() == 1 {
            dts.push("lib.rs.d.ts");
        } else {
            for seg in &segs[1..segs.len() - 1] {
                dts.push(seg);
            }
            dts.push(format!("{}.rs.d.ts", segs.last().unwrap()));
        }

        let body = ir.print_ts_mod(rust_curse::ir::ts::PrintTsModOpts {
            mod_id: mid,
            rel_src_path: &rel_src_path,
        });
        out.push('\n');
        out.push_str(&format!("// {}\n", dts.display()));
        out.push_str(&body);
    }
    out
}

pub fn print_wasm_js_per_mod<F>(ir: &rust_curse::ir::IR, mut printer: F) -> String
where
    F: FnMut(&rust_curse::ir::IR, rust_curse::ir::Id) -> String,
{
    let mut out = String::new();
    for (i, item) in ir.items.iter().enumerate() {
        if !matches!(item, rust_curse::ir::Item::Mod(_)) {
            continue;
        }
        let mid = rust_curse::ir::Id(i as u32);
        let mod_path = ir.get_mod_unchecked(mid).path.segments.join("/");
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("// $rust/{mod_path}\n"));
        out.push_str(&printer(ir, mid));
    }
    out
}

/// Set of all struct + fn ids in the IR. Used as the default "everything is
/// exposed" set for napi/wasm/js printer snapshots.
pub fn all_exposed_ids(ir: &rust_curse::ir::IR) -> HashSet<rust_curse::ir::Id> {
    ir.items
        .iter()
        .enumerate()
        .filter_map(|(i, it)| match it {
            rust_curse::ir::Item::Fn(_) | rust_curse::ir::Item::Struct(_) => {
                Some(rust_curse::ir::Id(i as u32))
            }
            _ => None,
        })
        .collect()
}

/// Resolve a list of item names into a set of ids by matching the last
/// segment of each item's path. Panics on missing or ambiguous names so
/// fixtures fail loudly.
pub fn exposed_by_name(ir: &rust_curse::ir::IR, names: &[&str]) -> HashSet<rust_curse::ir::Id> {
    let mut out = HashSet::new();
    for name in names {
        let matches: Vec<_> = ir
            .items
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                let path = match it {
                    rust_curse::ir::Item::Fn(def) => &def.path,
                    rust_curse::ir::Item::Struct(def) => &def.path,
                    rust_curse::ir::Item::Mod(def) => &def.path,
                };
                path.name() == Some(*name)
            })
            .map(|(i, _)| rust_curse::ir::Id(i as u32))
            .collect();
        match matches.as_slice() {
            [] => panic!("no item named `{name}` in IR"),
            [id] => {
                out.insert(*id);
            }
            many => panic!("name `{name}` ambiguous: {} matches", many.len()),
        }
    }
    out
}

#[macro_export]
macro_rules! rustdoc {
    ($($tt:tt)*) => {{
        let (_dir, _root, crate_) = $crate::common::gen_rustdoc(stringify!($($tt)*));
        crate_
    }};
}

#[macro_export]
macro_rules! ir {
    ($($tt:tt)*) => {{
        let (_dir, _root, crate_) = $crate::common::gen_rustdoc(stringify!($($tt)*));
        let mut warns = rust_curse::warns::Warns::new();
        rust_curse::ir::IR::from_rustdoc(&crate_, &mut warns).unwrap()
    }};
}

/// Render all IR-target snapshots in a single markdown body:
/// `Source` (Rust input), then `wasm`, `napi`, `ts`, `js-wasm`, `js-napi`,
/// `warns`, and `ir` debug at the bottom.
pub fn build_unified_snap(raw_src: &str) -> String {
    use rust_curse::warns::WarnsRenderer;

    let src = fmt_src(raw_src);
    let (_dir, project_root, crate_) = gen_rustdoc(&src);
    let mut warns = rust_curse::warns::Warns::new();
    let ir = rust_curse::ir::IR::from_rustdoc(&crate_, &mut warns).expect("build IR");
    let exposed = all_exposed_ids(&ir);

    let wasm_rs = fmt_src(&ir.print_wasm(&exposed));
    let napi_rs = fmt_src(&ir.print_napi(&exposed));
    let ts_out = print_ts_per_mod(&ir, PathBuf::from("src"));
    let wasm_js = print_wasm_js_per_mod(&ir, |ir, mid| {
        ir.print_wasm_js(mid, "$test".to_owned(), &exposed)
    });
    let napi_js = print_wasm_js_per_mod(&ir, |ir, mid| {
        ir.print_napi_js(mid, "$test".to_owned(), &exposed)
    });
    let warns_text = if warns.is_empty() {
        "(no warnings)\n".to_string()
    } else {
        let rendered = warns
            .render(&project_root, rust_curse::warns::Theme::UnicodeNoColor)
            .expect("render warns");
        let mut out = String::new();
        for w in rendered {
            if let Some(text) = w.rendered {
                out.push_str(&text);
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
        out
    };
    let ir_dbg = format!("{ir:#?}");

    let mut body = String::new();
    push_section(&mut body, "Source", "rs", &src);
    push_section(&mut body, "wasm", "rs", &wasm_rs);
    push_section(&mut body, "napi", "rs", &napi_rs);
    push_section(&mut body, "ts", "ts", &ts_out);
    push_section(&mut body, "js-wasm", "js", &wasm_js);
    push_section(&mut body, "js-napi", "js", &napi_js);
    push_section(&mut body, "warns", "text", &warns_text);
    push_section(&mut body, "ir", "rs", &ir_dbg);
    body
}

fn push_section(body: &mut String, name: &str, lang: &str, content: &str) {
    body.push_str(&format!("# {name}\n\n"));
    push_block(body, lang, content);
    body.push('\n');
}

fn push_block(body: &mut String, lang: &str, content: &str) {
    body.push_str("```");
    body.push_str(lang);
    body.push('\n');
    body.push_str(content);
    if !content.ends_with('\n') {
        body.push('\n');
    }
    body.push_str("```\n");
}

/// One unified snapshot covering every IR target plus IR debug.
///
/// Usage: `assert_snap! { <rust source> };`
#[macro_export]
macro_rules! assert_snap {
    ($($tt:tt)*) => {{
        let body = $crate::common::build_unified_snap(stringify!($($tt)*));
        insta::with_settings!({ snapshot_path => "ir/snapshots" }, {
            insta::assert_snapshot!(body);
        });
    }};
}

/// Assert that the build succeeded and at least one warning with the given
/// diagnostic code was emitted.
#[macro_export]
macro_rules! assert_warns {
    ({ $($tt:tt)* }, $code:expr) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let (_dir, _root, crate_) = $crate::common::gen_rustdoc(&src);
        let mut warns = rust_curse::warns::Warns::new();
        rust_curse::ir::IR::from_rustdoc(&crate_, &mut warns).expect("build succeeded");
        let expected: &str = $code;
        let found = warns.iter().any(|w| w.code == expected);
        assert!(
            found,
            "expected warning with code `{}`, got codes: {:?}",
            expected,
            warns.iter().map(|w| w.code.as_str()).collect::<Vec<_>>()
        );
    }};
}
