//! Shared helpers for scaffold tests: tempdir setup, dir-to-md
//! serialisation for snapshots, common Cargo.toml fixtures.

#![allow(dead_code)]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use rust_curse::ir::{Export, FnDef, Id, Item, ItemPath, ModDef, Type, IR};
use rust_curse::scaffold::{Scaffold, ScaffoldOptions};

pub const TEMP_FOLDER: &str = ".rust-curse";

pub const MIN_CARGO_TOML: &str = "[package]\n\
                                  name = \"test_crate\"\n\
                                  version = \"0.0.0\"\n\
                                  edition = \"2021\"\n";

pub const WORKSPACE_CARGO_TOML: &str = "[package]\n\
                                        name = \"test_crate\"\n\
                                        version.workspace = true\n\
                                        edition.workspace = true\n\
                                        \n\
                                        [dependencies]\n\
                                        napi = { workspace = true }\n";

pub struct TestProject {
    pub abs_root_path: PathBuf,
    pub abs_temp_path: PathBuf,
    _tempdir: tempfile::TempDir,
}

impl TestProject {
    pub fn new() -> Self {
        Self::with_cargo_toml(MIN_CARGO_TOML)
    }

    pub fn with_cargo_toml(cargo: &str) -> Self {
        let dir = tempfile::tempdir().expect("create tempdir");
        let abs_root_path = dir.path().to_path_buf();
        fs::write(abs_root_path.join("Cargo.toml"), cargo).expect("write Cargo.toml");
        let abs_temp_path = abs_root_path.join(TEMP_FOLDER);
        Self {
            abs_root_path,
            abs_temp_path,
            _tempdir: dir,
        }
    }

    pub fn scaffold(&self) -> Scaffold {
        Scaffold::new(ScaffoldOptions {
            abs_root_path: self.abs_root_path.clone(),
            temp_folder_rel_path: TEMP_FOLDER.into(),
            cargo_toml_rel_path: PathBuf::from("Cargo.toml"),
        })
        .expect("scaffold init")
    }

    /// Build insta settings that redact the per-run tempdir prefix from
    /// snapshots so paths like `/var/.../T/abc123/foo.rs` render as
    /// `<root>/foo.rs`.
    pub fn snapshot_settings(&self) -> insta::Settings {
        let mut settings = insta::Settings::clone_current();
        let abs_root = self.abs_root_path.to_string_lossy().to_string();
        settings.add_filter(&regex_escape(&abs_root), "<root>");
        settings
    }
}

fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' | '\\'
            | '/' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Walk the project root and emit every file as a fenced markdown block.
/// Output order is deterministic (sorted by file name at every level).
pub fn project_to_md(project: &TestProject) -> String {
    let mut entries: Vec<PathBuf> = Vec::new();
    collect_entries(&project.abs_root_path, &project.abs_root_path, &mut entries);
    entries.sort();

    let mut out = String::new();
    for rel in entries {
        let abs = project.abs_root_path.join(&rel);
        let content = match fs::read_to_string(&abs) {
            Ok(s) => s,
            Err(_) => "<binary or unreadable>".into(),
        };
        let lang = detect_lang(&rel);
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
        if name == "target" || name == ".git" || name == "Cargo.lock" {
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

fn detect_lang(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rs",
        Some("toml") => "toml",
        Some("json") => "json",
        Some("md") => "md",
        _ => "text",
    }
}

/// Description of a single function inside an orphan module fixture.
pub struct FnFixture<'a> {
    pub name: &'a str,
    pub exposed: bool,
}

/// Description of one orphan module's IR contribution: its `mod_name` (as
/// a `::`-separated path) and the list of exported functions.
pub struct ModFixture<'a> {
    pub mod_name: &'a str,
    pub fns: &'a [FnFixture<'a>],
}

/// Build a synthetic IR + exposed-id set for scaffold tests. Each module
/// fixture becomes one `Item::Mod` whose `path.segments` are the crate
/// name followed by the unmangled `mod_name` segments (matching what
/// real rustdoc IR produces), plus one `Item::Fn` per declared function.
/// A fn marked `exposed: true` is included in the returned `HashSet<Id>`.
pub fn make_ir(crate_name: &str, modules: &[ModFixture]) -> (IR, HashSet<Id>) {
    let mut ir = IR {
        crate_name: crate_name.to_string(),
        root: Id(0),
        items: Vec::new(),
    };
    let mut exposed: HashSet<Id> = HashSet::new();

    let root_id = Id(ir.items.len() as u32);
    ir.items.push(Item::Mod(ModDef {
        path: ItemPath::new(vec![crate_name.to_string()]),
        exports: Vec::new(),
    }));
    ir.root = root_id;

    let mut root_exports: Vec<Export> = Vec::new();

    for module in modules {
        let mod_segs: Vec<String> = std::iter::once(crate_name.to_string())
            .chain(module.mod_name.split("::").map(String::from))
            .collect();
        let mod_id = Id(ir.items.len() as u32);
        ir.items.push(Item::Mod(ModDef {
            path: ItemPath::new(mod_segs.clone()),
            exports: Vec::new(),
        }));

        let mut mod_exports: Vec<Export> = Vec::new();
        for fn_fix in module.fns {
            let mut fn_segs = mod_segs.clone();
            fn_segs.push(fn_fix.name.to_string());
            let fn_id = Id(ir.items.len() as u32);
            ir.items.push(Item::Fn(FnDef {
                path: ItemPath::new(fn_segs),
                params: Vec::new(),
                return_type: Type::Unit,
                docs: None,
            }));
            if fn_fix.exposed {
                exposed.insert(fn_id);
            }
            mod_exports.push(Export {
                name: fn_fix.name.to_string(),
                target: fn_id,
            });
        }
        if let Item::Mod(m) = &mut ir.items[mod_id.0 as usize] {
            m.exports = mod_exports;
        }

        root_exports.push(Export {
            name: module.mod_name.to_string(),
            target: mod_id,
        });
    }

    if let Item::Mod(m) = &mut ir.items[root_id.0 as usize] {
        m.exports = root_exports;
    }

    (ir, exposed)
}

/// Convenience wrapper for building an empty IR (for tests that just need
/// to exercise the scaffold with no modules).
pub fn empty_ir(crate_name: &str) -> (IR, HashSet<Id>) {
    make_ir(crate_name, &[])
}
