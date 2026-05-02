use std::fs;
use std::path::{Path, PathBuf};

use insta::Settings;

use rust_curse::orphans::{scan, OrphanScanError, OrphanScanOpts, SourceFilter};

fn snapshot_settings(abs_root: &Path) -> Settings {
    let mut settings = Settings::clone_current();
    let abs_root_str = abs_root.to_string_lossy().to_string();
    settings.add_filter(&regex_escape(&abs_root_str), "<root>");
    settings.set_description(dir_to_md(abs_root));
    settings
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

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn dir_to_md(abs_root: &Path) -> String {
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

#[test]
fn modern_layout_skips_submodules() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");

    write(&src.join("parent.rs"), "pub mod child;");
    write(&src.join("parent/child.rs"), "");
    write(&src.join("other.rs"), "");

    let abs_root = fs::canonicalize(&src).unwrap();

    let modules = scan(OrphanScanOpts {
        project_root_abs_path: &abs_root,
        src_rel_path: Path::new(""),
        rs_filter: SourceFilter::default(),
    })
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(modules));
}

#[test]
fn rs_exclude_filter_drops_module() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");

    write(&src.join("keep.rs"), "");
    write(&src.join("drop.rs"), "");

    let abs_root = fs::canonicalize(&src).unwrap();
    let exclude = vec!["drop.rs".to_string()];

    let modules = scan(OrphanScanOpts {
        project_root_abs_path: &abs_root,
        src_rel_path: Path::new(""),
        rs_filter: SourceFilter {
            include: &[],
            exclude: &exclude,
        },
    })
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(modules));
}

#[test]
fn conflict_errors_on_abc_rs_and_abc_mod_rs() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");

    write(&src.join("abc.rs"), "");
    write(&src.join("abc/mod.rs"), "");

    let abs_root = fs::canonicalize(&src).unwrap();

    let err = scan(OrphanScanOpts {
        project_root_abs_path: &abs_root,
        src_rel_path: Path::new(""),
        rs_filter: SourceFilter::default(),
    })
    .unwrap_err();

    assert!(matches!(err, OrphanScanError::Conflict { .. }), "{err:?}");
}
