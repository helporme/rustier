//! Shared test helpers.

#![allow(dead_code)]

use std::collections::HashSet;
use std::fs;

use rust_curse::*;

const TEST_CRATE_NAME: &str = "rustdoc_test_crate";

pub fn gen_rustdoc(src: &str) -> rustdoc_types::Crate {
    let project_dir = tempfile::tempdir().expect("create tempdir");
    let project_abs_path = project_dir.path();

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

    rustdoc::run(project_abs_path, &target_abs_dir, TEST_CRATE_NAME)
        .unwrap_or_else(|e| panic!("rustdoc generation failed: {e}"))
}

pub fn fmt_src(src: &str) -> String {
    match syn::parse_str::<syn::File>(src) {
        Ok(file) => prettyplease::unparse(&file),
        Err(_) => src.to_string(),
    }
}

pub fn print_js_per_mod<F>(ir: &rust_curse::ir::IR, mut printer: F) -> String
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
    ($($tt:tt)*) => {
        $crate::common::gen_rustdoc(stringify!($($tt)*))
    };
}

#[macro_export]
macro_rules! ir {
    ($($tt:tt)*) => {{
        let crate_ = $crate::common::gen_rustdoc(stringify!($($tt)*));
        rust_curse::ir::IR::try_from(&crate_).unwrap()
    }};
}

/// Snapshot the IR debug output, showing the Rust source in the review UI
/// when pressing `i`.
#[macro_export]
macro_rules! assert_ir_snap {
    ($($tt:tt)*) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let crate_ = $crate::common::gen_rustdoc(&src);
        let ir = rust_curse::ir::IR::try_from(&crate_).unwrap();
        insta::with_settings!({ description => src }, {
            insta::assert_debug_snapshot!(ir);
        });
    }};
}

/// Build IR and snapshot the TypeScript output. TS prints the full public
/// surface, so there is no exposed-ids parameter.
#[macro_export]
macro_rules! assert_ts_snap {
    ($($tt:tt)*) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let ir = {
            let crate_ = $crate::common::gen_rustdoc(&src);
            rust_curse::ir::IR::try_from(&crate_).unwrap()
        };
        insta::with_settings!({ description => src }, {
            insta::assert_snapshot!(ir.print_ts());
        });
    }};
}

/// Build IR and snapshot the napi printer output with all items exposed,
/// formatted through `prettyplease` for syntactic-validity verification.
#[macro_export]
macro_rules! assert_napi_snap {
    ($($tt:tt)*) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let ir = {
            let crate_ = $crate::common::gen_rustdoc(&src);
            rust_curse::ir::IR::try_from(&crate_).unwrap()
        };
        let exposed = $crate::common::all_exposed_ids(&ir);
        let napi_src = ir.print_napi(&exposed);
        let formatted = $crate::common::fmt_src(&napi_src);
        insta::with_settings!({ description => src }, {
            insta::assert_snapshot!(formatted);
        });
    }};
}

/// Like `assert_napi_snap!` but exposes only items whose name matches one
/// of the given identifiers. Use to snapshot tree-shaking pruning.
#[macro_export]
macro_rules! assert_napi_snap_exposed {
    ([$($name:literal),+] => { $($tt:tt)* }) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let ir = {
            let crate_ = $crate::common::gen_rustdoc(&src);
            rust_curse::ir::IR::try_from(&crate_).unwrap()
        };
        let exposed = $crate::common::exposed_by_name(&ir, &[$($name),+]);
        let napi_src = ir.print_napi(&exposed);
        let formatted = $crate::common::fmt_src(&napi_src);
        insta::with_settings!({ description => src }, {
            insta::assert_snapshot!(formatted);
        });
    }};
}

/// Build IR and snapshot the wasm-bindgen printer output with all items
/// exposed, formatted through `prettyplease`.
#[macro_export]
macro_rules! assert_wasm_snap {
    ($($tt:tt)*) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let ir = {
            let crate_ = $crate::common::gen_rustdoc(&src);
            rust_curse::ir::IR::try_from(&crate_).unwrap()
        };
        let exposed = $crate::common::all_exposed_ids(&ir);
        let wasm_src = ir.print_wasm(&exposed);
        let formatted = $crate::common::fmt_src(&wasm_src);
        insta::with_settings!({ description => src }, {
            insta::assert_snapshot!(formatted);
        });
    }};
}

#[macro_export]
macro_rules! assert_wasm_snap_exposed {
    ([$($name:literal),+] => { $($tt:tt)* }) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let ir = {
            let crate_ = $crate::common::gen_rustdoc(&src);
            rust_curse::ir::IR::try_from(&crate_).unwrap()
        };
        let exposed = $crate::common::exposed_by_name(&ir, &[$($name),+]);
        let wasm_src = ir.print_wasm(&exposed);
        let formatted = $crate::common::fmt_src(&wasm_src);
        insta::with_settings!({ description => src }, {
            insta::assert_snapshot!(formatted);
        });
    }};
}

/// Build IR and snapshot the napi-targeted JS re-export printer output for
/// every module, separated by `// $rust/<path>` headers.
#[macro_export]
macro_rules! assert_js_snap {
    ($($tt:tt)*) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let ir = {
            let crate_ = $crate::common::gen_rustdoc(&src);
            rust_curse::ir::IR::try_from(&crate_).unwrap()
        };
        let exposed = $crate::common::all_exposed_ids(&ir);
        let out = $crate::common::print_js_per_mod(&ir, |ir, mid| {
            ir.print_js(mid, "$test".to_owned(), &exposed)
        });
        insta::with_settings!({ description => src }, {
            insta::assert_snapshot!(out);
        });
    }};
}

/// Assert that building IR returns `Err(pat)`.
#[macro_export]
macro_rules! assert_ir_err {
    ({ $($tt:tt)* }, $pat:pat) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let crate_ = $crate::common::gen_rustdoc(&src);
        let result = rust_curse::ir::IR::try_from(&crate_);
        match result {
            Err(e) => assert!(
                matches!(e, $pat),
                "expected error {}, got {:?}",
                stringify!($pat),
                e
            ),
            Ok(ir) => panic!("expected error {}, got Ok({:?})", stringify!($pat), ir),
        }
    }};
}
