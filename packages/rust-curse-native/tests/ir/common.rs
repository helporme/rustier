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

pub fn allowed_by_name(
    crate_: &rustdoc_types::Crate,
    names: &[&str],
) -> HashSet<rustdoc_types::Id> {
    let mut out = HashSet::new();
    for name in names {
        let matches: Vec<_> = crate_
            .index
            .iter()
            .filter(|(_, it)| it.name.as_deref() == Some(*name))
            .map(|(id, _)| *id)
            .collect();
        match matches.as_slice() {
            [] => panic!("no item named `{name}` in crate"),
            [id] => {
                out.insert(*id);
            }
            many => panic!("name `{name}` ambiguous: {} matches", many.len()),
        }
    }
    out
}

pub fn fmt_src(src: &str) -> String {
    match syn::parse_str::<syn::File>(src) {
        Ok(file) => prettyplease::unparse(&file),
        Err(_) => src.to_string(),
    }
}

pub fn print_js_per_mod<F>(ir: &rust_curse::ir::IR, mut printer: F) -> String
where
    F: FnMut(&rust_curse::ir::IR, rust_curse::ir::ModId) -> String,
{
    let mut out = String::new();
    for i in 0..ir.mods.len() {
        let mid = rust_curse::ir::ModId(i as u32);
        let mod_path = ir.mod_(mid).path.segments.join("/");
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("// $rust/{mod_path}\n"));
        out.push_str(&printer(ir, mid));
    }
    out
}

pub fn all_leaf_ids(crate_: &rustdoc_types::Crate) -> HashSet<rustdoc_types::Id> {
    crate_
        .index
        .iter()
        .filter(|(_, item)| {
            !matches!(
                item.inner,
                rustdoc_types::ItemEnum::Module(_) | rustdoc_types::ItemEnum::ExternCrate { .. }
            )
        })
        .map(|(id, _)| *id)
        .collect()
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
        let allowed = $crate::common::all_leaf_ids(&crate_);
        rust_curse::ir::from_rustdoc::build_ir(&allowed, &crate_).unwrap()
    }};
}

/// Snapshot the IR debug output for specific named items, showing the Rust
/// source in the review UI when pressing `i`.
#[macro_export]
macro_rules! assert_ir_snap {
    ([$($name:literal),+] => { $($tt:tt)* }) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let crate_ = $crate::common::gen_rustdoc(&src);
        let allowed = $crate::common::allowed_by_name(&crate_, &[$($name),+]);
        let ir = rust_curse::ir::from_rustdoc::build_ir(&allowed, &crate_).unwrap();
        insta::with_settings!({ description => src }, {
            insta::assert_debug_snapshot!(ir);
        });
    }};
}

/// Build IR from all items and snapshot the TypeScript output, showing the
/// Rust source in the review UI when pressing `i`.
#[macro_export]
macro_rules! assert_ts_snap {
    ($($tt:tt)*) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let ir = {
            let crate_ = $crate::common::gen_rustdoc(&src);
            let allowed = $crate::common::all_leaf_ids(&crate_);
            rust_curse::ir::from_rustdoc::build_ir(&allowed, &crate_).unwrap()
        };
        insta::with_settings!({ description => src }, {
            insta::assert_snapshot!(ir.print_ts());
        });
    }};
}

/// Build IR from all items and snapshot the napi printer output,
/// formatted through `prettyplease` so the snapshot is canonical Rust
/// and we get a syntactic-validity check for free.
#[macro_export]
macro_rules! assert_napi_snap {
    ($($tt:tt)*) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let ir = {
            let crate_ = $crate::common::gen_rustdoc(&src);
            let allowed = $crate::common::all_leaf_ids(&crate_);
            rust_curse::ir::from_rustdoc::build_ir(&allowed, &crate_).unwrap()
        };
        let napi_src = ir.print_napi();
        let formatted = $crate::common::fmt_src(&napi_src);
        insta::with_settings!({ description => src }, {
            insta::assert_snapshot!(formatted);
        });
    }};
}

/// Build IR from all items and snapshot the wasm-bindgen printer output,
/// formatted through `prettyplease` so the snapshot is canonical Rust
/// and we get a syntactic-validity check for free.
#[macro_export]
macro_rules! assert_wasm_snap {
    ($($tt:tt)*) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let ir = {
            let crate_ = $crate::common::gen_rustdoc(&src);
            let allowed = $crate::common::all_leaf_ids(&crate_);
            rust_curse::ir::from_rustdoc::build_ir(&allowed, &crate_).unwrap()
        };
        let wasm_src = ir.print_wasm();
        let formatted = $crate::common::fmt_src(&wasm_src);
        insta::with_settings!({ description => src }, {
            insta::assert_snapshot!(formatted);
        });
    }};
}

/// Build IR from all items and snapshot the napi-targeted JS re-export
/// printer output for every module, separated by `// $rust/<path>` headers
/// so one fixture exercises the whole module tree.
#[macro_export]
macro_rules! assert_js_snap {
    ($($tt:tt)*) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let ir = {
            let crate_ = $crate::common::gen_rustdoc(&src);
            let allowed = $crate::common::all_leaf_ids(&crate_);
            rust_curse::ir::from_rustdoc::build_ir(&allowed, &crate_).unwrap()
        };
        let out = $crate::common::print_js_per_mod(&ir, |ir, mid| ir.print_js(mid, "$test".to_owned()));
        insta::with_settings!({ description => src }, {
            insta::assert_snapshot!(out);
        });
    }};
}

/// Like `assert_ir_snap!` but allows every leaf item. Use when name-based
/// filtering is ambiguous (e.g. two items share a name in different modules).
#[macro_export]
macro_rules! assert_ir_snap_all {
    ({ $($tt:tt)* }) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let crate_ = $crate::common::gen_rustdoc(&src);
        let allowed = $crate::common::all_leaf_ids(&crate_);
        let ir = rust_curse::ir::from_rustdoc::build_ir(&allowed, &crate_).unwrap();
        insta::with_settings!({ description => src }, {
            insta::assert_debug_snapshot!(ir);
        });
    }};
}

/// Assert that building IR returns `Err(pat)`.
#[macro_export]
macro_rules! assert_ir_err {
    ([$($name:literal),+] => { $($tt:tt)* }, $pat:pat) => {{
        let src = $crate::common::fmt_src(stringify!($($tt)*));
        let crate_ = $crate::common::gen_rustdoc(&src);
        let allowed = $crate::common::allowed_by_name(&crate_, &[$($name),+]);
        let result = rust_curse::ir::from_rustdoc::build_ir(&allowed, &crate_);
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
