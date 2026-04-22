//! Shared test helpers.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use rust_curse::discovery::rustdoc::run_cargo_rustdoc_json;

const TEST_CRATE_NAME: &str = "rustdoc_test_crate";

fn shared_target_abs_dir() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| std::env::temp_dir().join("rust_curse_rustdoc_test_cache"))
}

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

    let target_abs_dir = shared_target_abs_dir();
    fs::create_dir_all(target_abs_dir).expect("create shared target dir");

    run_cargo_rustdoc_json(project_abs_path, target_abs_dir, TEST_CRATE_NAME)
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
        rust_curse::codegen::ir::from_rustdoc::build_ir(&allowed, &crate_).unwrap()
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
        let ir = rust_curse::codegen::ir::from_rustdoc::build_ir(&allowed, &crate_).unwrap();
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
            rust_curse::codegen::ir::from_rustdoc::build_ir(&allowed, &crate_).unwrap()
        };
        insta::with_settings!({ description => src }, {
            insta::assert_snapshot!(ir.print_ts());
        });
    }};
}
