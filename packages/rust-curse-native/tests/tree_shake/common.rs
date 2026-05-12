//! Shared helpers for tree-shake tests: tempdir fixture builder,
//! fixture-to-md serialisation for `cargo insta review`, and the
//! `snapshot!` macro used by every test.

#![allow(dead_code)]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use rust_curse::common::{mangle_module_name, FileFilter};
use rust_curse::ir::{Id, IR};
use rust_curse::orphans;
use rust_curse::tree_shake::{self, item_path, TreeShakeError, TreeShakeOpts};
use rust_curse::warns::Warns;

pub const TEST_CRATE: &str = "rustdoc_tree_shake_test";

/// One file in the test fixture.
pub struct File<'a> {
    pub rel_path: &'a str,
    pub content: &'a str,
}

/// Build a tempdir with the given files plus a Cargo.toml and a `src/lib.rs`
/// that declares the listed orphan rust modules so rustdoc can see them.
pub struct Fixture {
    pub abs_root: PathBuf,
    _temp: tempfile::TempDir,
}

impl Fixture {
    pub fn new(mod_root: &str, orphan_rel_within_root: &[&str], files: &[File]) -> Self {
        let temp = tempfile::tempdir().expect("create tempdir");
        let abs_root = temp.path().to_path_buf();

        let cargo_toml = format!(
            "[package]\n\
             name = \"{TEST_CRATE}\"\n\
             version = \"0.0.0\"\n\
             edition = \"2021\"\n\
             publish = false\n\
             \n\
             [lib]\n\
             path = \"lib.rs\"\n\
             \n\
             [workspace]\n"
        );
        write_file(&abs_root.join("Cargo.toml"), &cargo_toml);

        let mut lib_rs = String::new();
        for orphan_rel in orphan_rel_within_root {
            let mod_name = orphans::gen_module_name(Path::new(orphan_rel));
            let mangled = mangle_module_name(&mod_name);
            let project_rel = format!("{mod_root}/{orphan_rel}");
            lib_rs.push_str(&format!("#[path = \"{project_rel}\"]\n"));
            lib_rs.push_str("#[allow(non_snake_case)]\n");
            lib_rs.push_str(&format!("pub mod {mangled};\n"));
        }
        write_file(&abs_root.join("lib.rs"), &lib_rs);

        for file in files {
            write_file(&abs_root.join(file.rel_path), file.content);
        }

        Fixture {
            abs_root,
            _temp: temp,
        }
    }

    pub fn run_shake(
        &self,
        src_rel: &str,
        js_include: &[String],
    ) -> Result<(IR, HashSet<Id>), TreeShakeError> {
        let src_rel_path = PathBuf::from(src_rel);

        let target_dir = self.abs_root.join("target");
        let crate_ =
            rust_curse::rustdoc::run(&self.abs_root, &target_dir, TEST_CRATE).expect("rustdoc gen");
        let mut warns = Warns::new();
        let ir = IR::from_rustdoc(&crate_, &mut warns).expect("build ir");

        let js_filter = FileFilter {
            include: Some(js_include.to_vec()),
            exclude: None,
        };
        let exposed = tree_shake::shake(
            &ir,
            TreeShakeOpts {
                project_root_abs_path: &self.abs_root,
                src_rel_path: &src_rel_path,
                js_filter: &js_filter,
            },
        )?;

        Ok((ir, exposed))
    }
}

fn write_file(abs_path: &Path, content: &str) {
    if let Some(parent) = abs_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(abs_path, content).unwrap();
}

/// Render every fixture file as a fenced markdown block, sorted by path,
/// so reviewers can see the on-disk layout in `cargo insta review`.
pub fn fixture_md(abs_root: &Path) -> String {
    let mut entries: Vec<PathBuf> = Vec::new();
    collect_entries(abs_root, abs_root, &mut entries);
    entries.sort();

    let mut out = String::new();
    for rel in entries {
        let abs = abs_root.join(&rel);
        let content = fs::read_to_string(&abs).unwrap_or_else(|_| "<unreadable>".into());
        let lang = match rel.extension().and_then(|e| e.to_str()) {
            Some("rs") => "rs",
            Some("js" | "ts" | "tsx" | "mjs") => "js",
            Some("toml") => "toml",
            _ => "text",
        };
        let rel_display = rel.to_string_lossy().replace('\\', "/");
        out.push('`');
        out.push_str(&rel_display);
        out.push('`');
        out.push('\n');
        out.push_str("```");
        out.push_str(lang);
        out.push('\n');
        out.push_str(&content);
        if !content.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("```\n\n");
    }
    out
}

fn collect_entries(abs_root: &Path, abs_dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read_dir) = fs::read_dir(abs_dir) else {
        return;
    };
    let mut local: Vec<PathBuf> = read_dir.flatten().map(|e| e.path()).collect();
    local.sort();
    for path in local {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "target" || name == "Cargo.lock" {
            continue;
        }
        if path.is_dir() {
            collect_entries(abs_root, &path, out);
        } else {
            let rel = path
                .strip_prefix(abs_root)
                .expect("path under root")
                .to_path_buf();
            out.push(rel);
        }
    }
}

pub fn snapshot_set(ir: &IR, ids: &HashSet<Id>) -> Vec<String> {
    let mut names: Vec<String> = ids.iter().map(|id| item_path(ir, *id)).collect();
    names.sort();
    names
}

#[macro_export]
macro_rules! snapshot {
    ($fixture:expr, $ir:expr, $exposed:expr) => {{
        let names = $crate::common::snapshot_set(&$ir, &$exposed);
        let md = $crate::common::fixture_md(&$fixture.abs_root);
        insta::with_settings!({ description => md }, {
            insta::assert_debug_snapshot!(names);
        });
    }};
}
