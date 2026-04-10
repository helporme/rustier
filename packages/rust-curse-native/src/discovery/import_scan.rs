//! Scans JS/TS files for ESM imports that reference Rust modules.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use glob::glob;

use crate::model::RsImport;

/// Scans js-like (ts, jsx, tsx, svelte, etc.) files matched by `include`/`exclude` glob patterns for ESM imports
/// that reference `.rs` files under `abs_module_dir`.
pub fn scan_imports(
    abs_module_dir: &Path,
    include: &[String],
    exclude: &[String],
) -> Result<Vec<RsImport>, ImportScanError> {
    let excluded_paths = collect_glob_paths(exclude)?;
    let mut result = Vec::new();

    for pattern in include {
        let entries = glob(pattern).map_err(|source| ImportScanError::InvalidGlob {
            pattern: pattern.clone(),
            source,
        })?;

        for entry in entries.filter_map(|e| e.ok()) {
            let abs_path = match fs::canonicalize(&entry) {
                Ok(p) => p,
                Err(_) => continue,
            };

            if excluded_paths.contains(&abs_path) {
                continue;
            }

            let source = fs::read_to_string(&abs_path)
                .map_err(|e| ImportScanError::ReadFile(abs_path.clone(), e))?;

            let file_dir = abs_path.parent().unwrap_or(Path::new("."));
            result.extend(extract_imports(
                &source,
                &abs_path,
                file_dir,
                abs_module_dir,
            ));
        }
    }

    Ok(result)
}

fn collect_glob_paths(patterns: &[String]) -> Result<HashSet<PathBuf>, ImportScanError> {
    let mut paths = HashSet::new();
    for pattern in patterns {
        let entries = glob(pattern).map_err(|source| ImportScanError::InvalidGlob {
            pattern: pattern.clone(),
            source,
        })?;
        for entry in entries.filter_map(|e| e.ok()) {
            if let Ok(canonical) = fs::canonicalize(entry) {
                paths.insert(canonical);
            }
        }
    }
    Ok(paths)
}

fn extract_imports(
    source: &str,
    js_abs_path: &Path,
    file_dir: &Path,
    abs_module_dir: &Path,
) -> Vec<RsImport> {
    let cleaned = strip_comments(source);
    let mut results = Vec::new();

    for stmt in collect_import_statements(&cleaned) {
        if let Some(import) = parse_import_statement(&stmt, js_abs_path, file_dir, abs_module_dir) {
            results.push(import);
        }
    }

    results
}

/// Strip `//` line comments and `/* ... */` block comments from source.
fn strip_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut result = String::with_capacity(source.len());
    let mut i = 0;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            // Line comment -> skip to end of line
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            // Block comment -> skip to */
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < bytes.len() {
                i += 2; // skip */
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    result
}

/// Collect complete import/export..from statements, joining multiline ones.
fn collect_import_statements(source: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut pending: Option<String> = None;

    for line in source.lines() {
        let trimmed = line.trim();

        if let Some(ref mut buf) = pending {
            buf.push(' ');
            buf.push_str(trimmed);
            if buf.contains(" from ") && has_string_literal(buf) {
                statements.push(std::mem::take(buf));
                pending = None;
            }
        } else if trimmed.starts_with("import") || trimmed.starts_with("export") {
            if trimmed.contains(" from ") && has_string_literal(trimmed) {
                statements.push(trimmed.to_string());
            } else {
                pending = Some(trimmed.to_string());
            }
        }
    }

    statements
}

fn has_string_literal(s: &str) -> bool {
    (s.contains('"') && s.chars().filter(|&c| c == '"').count() >= 2)
        || (s.contains('\'') && s.chars().filter(|&c| c == '\'').count() >= 2)
}

fn parse_import_statement(
    stmt: &str,
    js_abs_path: &Path,
    file_dir: &Path,
    abs_module_dir: &Path,
) -> Option<RsImport> {
    let from_idx = stmt.rfind(" from ")?;

    // Extract keyword length: "import" or "export" (both 6 chars)
    let keyword_end = 6;
    let members_part = &stmt[keyword_end..from_idx];

    // Skip type-only imports: `import type ...` / `export type ...`
    if members_part.trim_start().starts_with("type") {
        let after_type = members_part.trim_start().strip_prefix("type")?;
        // `import type { ... }` or `import type Foo` -> skip
        // but NOT `import { type Foo, bar }` (that has `{` before `type`)
        let first_non_type = after_type.chars().next().unwrap_or(' ');
        if first_non_type == ' ' || first_non_type == '{' {
            return None;
        }
    }

    // Extract specifier from the first string literal after " from "
    let after_from = &stmt[from_idx + 6..];
    let specifier = extract_first_string_literal(after_from)?;

    let rs_rel_path = resolve_specifier(&specifier, file_dir, abs_module_dir)?;

    let members = parse_members(members_part);
    if members.is_empty() {
        return None;
    }

    Some(RsImport {
        js_abs_path: js_abs_path.to_path_buf(),
        rs_rel_path,
        members,
    })
}

/// Extract the first single- or double-quoted string literal from `s`.
fn extract_first_string_literal(s: &str) -> Option<String> {
    let dq = find_first_quoted(s, '"');
    let sq = find_first_quoted(s, '\'');

    match (dq, sq) {
        (Some((di, ds)), Some((si, ss))) => {
            if di < si {
                Some(ds)
            } else {
                Some(ss)
            }
        }
        (Some((_, s)), None) => Some(s),
        (None, Some((_, s))) => Some(s),
        (None, None) => None,
    }
}

fn find_first_quoted(s: &str, quote_char: char) -> Option<(usize, String)> {
    let start = s.find(quote_char)?;
    let rest = &s[start + 1..];
    let end = rest.find(quote_char)?;
    let content = &rest[..end];
    if content.is_empty() {
        None
    } else {
        Some((start, content.to_string()))
    }
}

/// Resolves a JS import specifier to a `rel_path` under `abs_module_dir`.
///
/// Tries in order:
/// 1. Exact path (e.g. `./greeting.rs`)
/// 2. With `.rs` appended (e.g. `./greeting` -> `./greeting.rs`)
/// 3. As directory module (e.g. `./hello` -> `./hello/mod.rs`)
fn resolve_specifier(specifier: &str, file_dir: &Path, abs_module_dir: &Path) -> Option<PathBuf> {
    let base = file_dir.join(specifier);

    // 1. Exact path
    if let Some(rel) = try_resolve(&base, abs_module_dir) {
        return Some(rel);
    }

    // 2. Append .rs
    if !specifier.ends_with(".rs") {
        let with_rs = base.with_extension("rs");
        if let Some(rel) = try_resolve(&with_rs, abs_module_dir) {
            return Some(rel);
        }

        // 3. Directory module -> dir/mod.rs
        let mod_rs = base.join("mod.rs");
        if let Some(rel) = try_resolve(&mod_rs, abs_module_dir) {
            return Some(rel);
        }
    }

    None
}

/// Canonicalize `candidate` and, if it is a file inside `abs_module_dir`,
/// return the relative path from `abs_module_dir`.
fn try_resolve(candidate: &Path, abs_module_dir: &Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(candidate).ok()?;
    if !canonical.is_file() {
        return None;
    }
    canonical
        .strip_prefix(abs_module_dir)
        .ok()
        .map(PathBuf::from)
}

fn parse_members(specifier_part: &str) -> Vec<String> {
    let trimmed = specifier_part.trim();

    if let (Some(open), Some(close)) = (trimmed.find('{'), trimmed.rfind('}')) {
        let inner = &trimmed[open + 1..close];
        return inner
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.split_whitespace().next().unwrap_or(s).to_string())
            .collect();
    }

    if trimmed.starts_with('*') {
        return vec!["*".to_string()];
    }

    // Default import (e.g. `import Foo from './file.rs'`)
    let ident = trimmed.split_whitespace().next().unwrap_or("").to_string();
    if !ident.is_empty() {
        return vec!["default".to_string()];
    }

    Vec::new()
}

#[derive(Debug, thiserror::Error)]
pub enum ImportScanError {
    #[error("invalid glob pattern '{pattern}'")]
    InvalidGlob {
        pattern: String,
        #[source]
        source: glob::PatternError,
    },

    #[error("failed to read '{}'", .0.display())]
    ReadFile(PathBuf, #[source] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_named_members() {
        assert_eq!(
            parse_members(" { add, subtract } "),
            vec!["add", "subtract"]
        );
    }

    #[test]
    fn parse_star_import() {
        assert_eq!(parse_members(" * "), vec!["*"]);
    }

    #[test]
    fn parse_default_import() {
        assert_eq!(parse_members(" Foo "), vec!["default"]);
    }

    #[test]
    fn parse_aliased_member() {
        assert_eq!(parse_members(" { add as myAdd } "), vec!["add"]);
        assert_eq!(
            parse_members(" { add as myAdd, remove } "),
            vec!["add", "remove"]
        );
    }

    #[test]
    fn parse_empty() {
        assert!(parse_members("").is_empty());
    }

    #[test]
    fn extract_first_double_quoted() {
        assert_eq!(
            extract_first_string_literal(r#""./bar.rs""#),
            Some("./bar.rs".into())
        );
    }

    #[test]
    fn extract_first_single_quoted() {
        assert_eq!(
            extract_first_string_literal("'./bar.rs'"),
            Some("./bar.rs".into())
        );
    }

    #[test]
    fn extract_first_ignores_trailing_literals() {
        // with/assert syntax puts extra quoted strings after the specifier
        assert_eq!(
            extract_first_string_literal(r#"'./bar.rs' with { type: 'rust' }"#),
            Some("./bar.rs".into())
        );
    }

    /// Sets up a module dir with .rs files and returns (abs_module_dir, js_dir).
    /// `extract_imports` needs real files on disk for `try_resolve` to canonicalize.
    fn setup_extract_dir() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let module_dir = dir.path().join("src");
        fs::create_dir_all(module_dir.join("nested")).unwrap();
        fs::write(module_dir.join("greet.rs"), "").unwrap();
        fs::write(module_dir.join("math.rs"), "").unwrap();
        fs::write(module_dir.join("nested/deep.rs"), "").unwrap();
        fs::create_dir_all(module_dir.join("utils")).unwrap();
        fs::write(module_dir.join("utils/mod.rs"), "").unwrap();

        let js_dir = dir.path().join("app");
        fs::create_dir_all(&js_dir).unwrap();

        let abs_module_dir = fs::canonicalize(&module_dir).unwrap();
        (dir, abs_module_dir, js_dir)
    }

    #[test]
    fn extract_named_import() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import { hello } from '../src/greet.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("greet.rs"));
        assert_eq!(imports[0].members, vec!["hello"]);
    }

    #[test]
    fn extract_multiple_members() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import { add, subtract } from '../src/math.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("math.rs"));
        assert_eq!(imports[0].members, vec!["add", "subtract"]);
    }

    #[test]
    fn extract_star_import() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import * from '../src/greet.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].members, vec!["*"]);
    }

    #[test]
    fn extract_extensionless_resolves_to_rs() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import { add } from '../src/math'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("math.rs"));
    }

    #[test]
    fn extract_extensionless_nested_module() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import { go } from '../src/nested/deep'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("nested/deep.rs"));
    }

    #[test]
    fn extract_extensionless_multiline() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "\
import {
  add,
} from '../src/math'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("math.rs"));
    }

    #[test]
    fn extract_extensionless_with_syntax() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import { hello } from '../src/greet' with { type: 'rust' }";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("greet.rs"));
    }

    #[test]
    fn extract_directory_module() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import { help } from '../src/utils'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("utils/mod.rs"));
    }

    #[test]
    fn extract_nested_module() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import { go } from '../src/nested/deep.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("nested/deep.rs"));
    }

    #[test]
    fn extract_export_reexport() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("index.js");
        let source = "export { hello } from '../src/greet.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("greet.rs"));
        assert_eq!(imports[0].members, vec!["hello"]);
    }

    #[test]
    fn extract_multiple_lines() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "\
import { hello } from '../src/greet.rs'
import { add } from '../src/math.rs'
const x = 42;
import { go } from '../src/nested/deep.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("greet.rs"));
        assert_eq!(imports[1].rs_rel_path, PathBuf::from("math.rs"));
        assert_eq!(imports[2].rs_rel_path, PathBuf::from("nested/deep.rs"));
    }

    #[test]
    fn extract_skips_non_module_imports() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "\
import React from 'react'
import { foo } from './local.js'
const bar = require('./baz')";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);
        assert!(imports.is_empty());
    }

    #[test]
    fn extract_aliased_members() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import { hello as greet, add as plus } from '../src/greet.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].members, vec!["hello", "add"]);
    }

    #[test]
    fn extract_ignores_type_import() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import type { Foo } from '../src/greet.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);
        assert!(imports.is_empty());
    }

    #[test]
    fn extract_ignores_type_default_import() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import type Foo from '../src/greet.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);
        assert!(imports.is_empty());
    }

    #[test]
    fn extract_ignores_export_type() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("index.js");
        let source = "export type { Foo } from '../src/greet.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);
        assert!(imports.is_empty());
    }

    #[test]
    fn extract_with_assert_syntax() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import { hello } from '../src/greet.rs' assert { type: 'rust' }";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("greet.rs"));
        assert_eq!(imports[0].members, vec!["hello"]);
    }

    #[test]
    fn extract_with_with_syntax() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import { hello } from '../src/greet.rs' with { type: 'rust' }";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("greet.rs"));
        assert_eq!(imports[0].members, vec!["hello"]);
    }

    #[test]
    fn extract_multiline_import() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "\
import {
  hello,
} from '../src/greet.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("greet.rs"));
        assert_eq!(imports[0].members, vec!["hello"]);
    }

    #[test]
    fn extract_multiline_multiple_members() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "\
import {
  add,
  subtract,
} from '../src/math.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].members, vec!["add", "subtract"]);
    }

    #[test]
    fn extract_multiline_export() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("index.js");
        let source = "\
export {
  hello,
} from '../src/greet.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("greet.rs"));
    }

    #[test]
    fn extract_ignores_line_commented_import() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "// import { hello } from '../src/greet.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);
        assert!(imports.is_empty());
    }

    #[test]
    fn extract_ignores_block_commented_import() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "/* import { hello } from '../src/greet.rs' */";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);
        assert!(imports.is_empty());
    }

    #[test]
    fn extract_ignores_multiline_block_commented_import() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "\
/*
import { hello } from '../src/greet.rs'
import { add } from '../src/math.rs'
*/
import { go } from '../src/nested/deep.rs'";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("nested/deep.rs"));
    }

    #[test]
    fn extract_ignores_inline_comment_after_import() {
        let (_dir, abs_module_dir, js_dir) = setup_extract_dir();
        let js_file = js_dir.join("main.js");
        let source = "import { hello } from '../src/greet.rs' // greeting module";

        let imports = extract_imports(source, &js_file, &js_dir, &abs_module_dir);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("greet.rs"));
        assert_eq!(imports[0].members, vec!["hello"]);
    }

    #[test]
    fn scan_imports_from_temp_dir() {
        let dir = tempfile::tempdir().unwrap();

        // Create a .rs file in module dir
        let module_dir = dir.path().join("src");
        fs::create_dir_all(&module_dir).unwrap();
        fs::write(module_dir.join("greet.rs"), "pub fn hello() {}").unwrap();

        // Create a JS file that imports it
        let js_dir = dir.path().join("app");
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(
            js_dir.join("main.js"),
            "import { hello } from '../src/greet.rs'\n",
        )
        .unwrap();

        let abs_module_dir = fs::canonicalize(&module_dir).unwrap();
        let include = vec![format!("{}/**/*.js", js_dir.display())];

        let imports = scan_imports(&abs_module_dir, &include, &[]).unwrap();

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("greet.rs"));
        assert_eq!(imports[0].members, vec!["hello"]);
    }

    #[test]
    fn scan_extensionless_import() {
        let dir = tempfile::tempdir().unwrap();

        let module_dir = dir.path().join("src");
        fs::create_dir_all(&module_dir).unwrap();
        fs::write(module_dir.join("math.rs"), "pub fn add() {}").unwrap();

        let js_dir = dir.path().join("app");
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(
            js_dir.join("main.js"),
            "import { add } from '../src/math'\n",
        )
        .unwrap();

        let abs_module_dir = fs::canonicalize(&module_dir).unwrap();
        let include = vec![format!("{}/**/*.js", js_dir.display())];

        let imports = scan_imports(&abs_module_dir, &include, &[]).unwrap();

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("math.rs"));
    }

    #[test]
    fn scan_directory_module_import() {
        let dir = tempfile::tempdir().unwrap();

        let module_dir = dir.path().join("src");
        fs::create_dir_all(module_dir.join("utils")).unwrap();
        fs::write(module_dir.join("utils/mod.rs"), "pub fn help() {}").unwrap();

        let js_dir = dir.path().join("app");
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(
            js_dir.join("main.js"),
            "import { help } from '../src/utils'\n",
        )
        .unwrap();

        let abs_module_dir = fs::canonicalize(&module_dir).unwrap();
        let include = vec![format!("{}/**/*.js", js_dir.display())];

        let imports = scan_imports(&abs_module_dir, &include, &[]).unwrap();

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].rs_rel_path, PathBuf::from("utils/mod.rs"));
    }

    #[test]
    fn scan_ignores_non_rs_imports() {
        let dir = tempfile::tempdir().unwrap();

        let module_dir = dir.path().join("src");
        fs::create_dir_all(&module_dir).unwrap();

        let js_dir = dir.path().join("app");
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(
            js_dir.join("main.js"),
            "import { foo } from './other.js'\nimport React from 'react'\n",
        )
        .unwrap();

        let abs_module_dir = fs::canonicalize(&module_dir).unwrap();
        let include = vec![format!("{}/**/*.js", js_dir.display())];

        let imports = scan_imports(&abs_module_dir, &include, &[]).unwrap();
        assert!(imports.is_empty());
    }

    #[test]
    fn scan_excludes_files() {
        let dir = tempfile::tempdir().unwrap();

        let module_dir = dir.path().join("src");
        fs::create_dir_all(&module_dir).unwrap();
        fs::write(module_dir.join("greet.rs"), "pub fn hello() {}").unwrap();

        let js_dir = dir.path().join("app");
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(
            js_dir.join("main.js"),
            "import { hello } from '../src/greet.rs'\n",
        )
        .unwrap();

        let abs_module_dir = fs::canonicalize(&module_dir).unwrap();
        let include = vec![format!("{}/**/*.js", js_dir.display())];
        let exclude = vec![format!("{}/main.js", js_dir.display())];

        let imports = scan_imports(&abs_module_dir, &include, &exclude).unwrap();
        assert!(imports.is_empty());
    }
}
