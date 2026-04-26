use std::fs;
use std::path::{Path, PathBuf};

use insta::Settings;

use rust_curse::orphans::{
    scan, shake, OrphanScanError, OrphanScanOpts, OrphanShakeOpts, SourceFilter,
};

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

/// Render every file under `abs_root` as a fenced markdown block, sorted by
/// path. Used as the snapshot description so reviewers can see the on-disk
/// fixture each test built.
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
fn flat_module_with_named_import() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("greet.rs"), "pub fn hello() {}");
    write(
        &app.join("main.js"),
        "import { hello } from '../src/greet.rs'\n",
    );

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let shaked = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &[],
            },
        },
    )
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(shaked));
}

#[test]
fn star_import_yields_all() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("math.rs"), "pub fn add() {}");
    write(&app.join("main.js"), "import * from '../src/math.rs'\n");

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let shaked = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &[],
            },
        },
    )
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(shaked));
}

#[test]
fn unimported_module_yields_none() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("greet.rs"), "");
    write(&src.join("unused.rs"), "");
    write(
        &app.join("main.js"),
        "import { hello } from '../src/greet.rs'\n",
    );

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let shaked = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &[],
            },
        },
    )
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(shaked));
}

#[test]
fn multiple_importers_merge_members() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("greet.rs"), "");
    write(
        &app.join("a.js"),
        "import { hello } from '../src/greet.rs'\n",
    );
    write(
        &app.join("b.js"),
        "import { goodbye } from '../src/greet.rs'\n",
    );

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let shaked = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &[],
            },
        },
    )
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(shaked));
}

#[test]
fn directory_module() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("utils/mod.rs"), "");
    write(
        &app.join("main.js"),
        "import { help } from '../src/utils'\n",
    );

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let shaked = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &[],
            },
        },
    )
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(shaked));
}

#[test]
fn nested_orphan() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("nested/deep.rs"), "");
    write(
        &app.join("main.js"),
        "import { go } from '../src/nested/deep.rs'\n",
    );

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let shaked = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &[],
            },
        },
    )
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(shaked));
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
        root_abs_path: &abs_root,
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
        root_abs_path: &abs_root,
        rs_filter: SourceFilter {
            include: &[],
            exclude: &exclude,
        },
    })
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(modules));
}

#[test]
fn js_exclude_filter_ignores_importer() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("greet.rs"), "");
    write(
        &app.join("main.js"),
        "import { hello } from '../src/greet.rs'\n",
    );

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];
    let exclude = vec![format!("{}/main.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let shaked = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &exclude,
            },
        },
    )
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(shaked));
}

#[test]
fn export_named_reexport() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("greet.rs"), "");
    write(
        &app.join("main.js"),
        "export { hello } from '../src/greet.rs'\n",
    );

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let shaked = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &[],
            },
        },
    )
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(shaked));
}

#[test]
fn aliased_import_uses_source_name() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("greet.rs"), "");
    write(
        &app.join("main.js"),
        "import { hello as hi, goodbye as bye } from '../src/greet.rs'\n",
    );

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let shaked = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &[],
            },
        },
    )
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(shaked));
}

#[test]
fn import_with_attribute() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("greet.rs"), "");
    write(
        &app.join("main.js"),
        "import { hello } from '../src/greet.rs' with { type: 'rust' }\n",
    );

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let shaked = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &[],
            },
        },
    )
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(shaked));
}

#[test]
fn import_assert_attribute() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("greet.rs"), "");
    write(
        &app.join("main.js"),
        "import { hello } from '../src/greet.rs' assert { type: 'rust' }\n",
    );

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let shaked = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &[],
            },
        },
    )
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(shaked));
}

#[test]
fn export_star_alias_yields_all() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("greet.rs"), "pub fn hello() {}");
    write(
        &app.join("main.js"),
        "export * as utils from '../src/greet.rs'\n",
    );

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let shaked = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &[],
            },
        },
    )
    .unwrap();

    snapshot_settings(dir.path()).bind(|| insta::assert_debug_snapshot!(shaked));
}

#[test]
fn default_import_errors_during_shake() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let app = dir.path().join("app");

    write(&src.join("greet.rs"), "");
    write(
        &app.join("main.js"),
        "import greet from '../src/greet.rs'\n",
    );

    let abs_root = fs::canonicalize(&src).unwrap();
    let include = vec![format!("{}/**/*.js", app.display())];

    let modules = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap();
    let err = shake(
        &modules,
        OrphanShakeOpts {
            root_abs_path: &abs_root,
            js_filter: SourceFilter {
                include: &include,
                exclude: &[],
            },
        },
    )
    .unwrap_err();

    assert!(
        matches!(err, OrphanScanError::DefaultImport { .. }),
        "{err:?}"
    );
}

#[test]
fn conflict_errors_on_abc_rs_and_abc_mod_rs() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");

    write(&src.join("abc.rs"), "");
    write(&src.join("abc/mod.rs"), "");

    let abs_root = fs::canonicalize(&src).unwrap();

    let err = scan(OrphanScanOpts {
        root_abs_path: &abs_root,
        rs_filter: SourceFilter::default(),
    })
    .unwrap_err();

    assert!(matches!(err, OrphanScanError::Conflict { .. }), "{err:?}");
}
