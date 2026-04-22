//! Step 2.
//! Scans JS/TS files for imports of discovered Rust modules.

use glob::glob;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::steps::s1_discovery::RustModule;

pub struct RustImport {
    pub js_file: String,
    /// Absolute path to the matched module file (e.g. the `path` field of `RustModule`)
    pub rs_file: String,
    pub members: Vec<String>,
}

/// Scans JS/TS files matched by `include`/`exclude` globs for imports that
/// reference any of the given Rust modules.
///
/// Matches imports ending in `.rs` (e.g. `from './greeting.rs'`) as well as
/// extensionless imports that resolve to a module directory with `mod.rs`
/// (e.g. `from './hello'` matching `hello/mod.rs`).
pub fn scan_for_imports_of_rust_modules_in_js(
    modules: &[RustModule],
    include: Vec<String>,
    exclude: Vec<String>,
) -> Vec<RustImport> {
    let index = build_module_index(modules);

    let excluded_paths: HashSet<PathBuf> = exclude
        .iter()
        .flat_map(|pattern| glob(pattern).ok().into_iter().flatten())
        .filter_map(|entry| entry.ok())
        .filter_map(|p| fs::canonicalize(p).ok())
        .collect();

    let mut result = Vec::new();

    for pattern in &include {
        let entries = match glob(pattern) {
            Ok(paths) => paths,
            Err(_) => continue,
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let canonical = match fs::canonicalize(&entry) {
                Ok(p) => p,
                Err(_) => continue,
            };

            if excluded_paths.contains(&canonical) {
                continue;
            }

            let source = match fs::read_to_string(&canonical) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let js_file = canonical.to_string_lossy().into_owned();
            let file_dir = canonical.parent().unwrap_or(Path::new("."));
            result.extend(extract_imports(&source, &js_file, file_dir, &index));
        }
    }

    result
}

/// Maps resolved absolute paths to the module's canonical path.
/// For each module, we index:
/// - The module file path itself (e.g. `/abs/path/greeting.rs`)
/// - If the module file is `mod.rs`, also the parent directory
///   (so `import './hello'` resolves to `hello/mod.rs`)
struct ModuleIndex {
    /// resolved absolute path → module's canonical path (the `path` field)
    lookup: HashMap<PathBuf, String>,
}

fn build_module_index(modules: &[RustModule]) -> ModuleIndex {
    let mut lookup = HashMap::new();

    for module in modules {
        let module_path = Path::new(&module.path);

        // Index the file itself: ./greeting.rs → module
        if let Ok(canonical) = fs::canonicalize(module_path) {
            lookup.insert(canonical.clone(), module.path.clone());

            // If the file is mod.rs, also index the parent directory
            // so `import './hello'` matches `hello/mod.rs`
            if module_path.file_name().is_some_and(|n| n == "mod.rs") {
                if let Some(parent) = canonical.parent() {
                    lookup.insert(parent.to_path_buf(), module.path.clone());
                }
            }
        }
    }

    ModuleIndex { lookup }
}

fn extract_imports(
    source: &str,
    js_file: &str,
    file_dir: &Path,
    index: &ModuleIndex,
) -> Vec<RustImport> {
    let mut results = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();

        if !trimmed.starts_with("import") && !trimmed.starts_with("export") {
            continue;
        }

        // Must have `from` keyword
        let from_idx = match trimmed.rfind(" from ") {
            Some(idx) => idx,
            None => continue,
        };

        let specifier = match extract_string_literal_from_end(trimmed) {
            Some(s) => s,
            None => continue,
        };

        // Try to resolve the specifier against the module index
        let rs_file = match resolve_specifier(&specifier, file_dir, index) {
            Some(path) => path,
            None => continue,
        };

        const KEYWORD_END: usize = 6;
        let members_part = &trimmed[KEYWORD_END..from_idx];
        let members = parse_members(members_part);

        if members.is_empty() {
            continue;
        }

        results.push(RustImport {
            js_file: js_file.to_string(),
            rs_file,
            members,
        });
    }

    results
}

/// Tries to resolve a JS import specifier to a known module path.
/// Checks in order:
/// 1. The specifier as-is (e.g. `./greeting.rs`)
/// 2. With `.rs` appended (e.g. `./greeting` → `./greeting.rs`)
/// 3. As a directory with `mod.rs` (e.g. `./hello` → `./hello/mod.rs`)
fn resolve_specifier(specifier: &str, file_dir: &Path, index: &ModuleIndex) -> Option<String> {
    let base = file_dir.join(specifier);

    // 1. Exact path (e.g. `./greeting.rs`)
    if let Ok(canonical) = fs::canonicalize(&base) {
        if let Some(module_path) = index.lookup.get(&canonical) {
            return Some(module_path.clone());
        }
    }

    // 2. Append .rs (e.g. `./greeting` → `./greeting.rs`)
    if !specifier.ends_with(".rs") {
        let with_rs = file_dir.join(format!("{}.rs", specifier));
        if let Ok(canonical) = fs::canonicalize(&with_rs) {
            if let Some(module_path) = index.lookup.get(&canonical) {
                return Some(module_path.clone());
            }
        }
    }

    // 3. Directory module (e.g. `./hello` → `./hello/` which maps to `hello/mod.rs`)
    if let Ok(canonical) = fs::canonicalize(&base) {
        if canonical.is_dir() {
            if let Some(module_path) = index.lookup.get(&canonical) {
                return Some(module_path.clone());
            }
        }
    }

    None
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

    let ident = trimmed.split_whitespace().next().unwrap_or("").to_string();
    if !ident.is_empty() {
        return vec!["default".to_string()];
    }

    Vec::new()
}

fn extract_string_literal_from_end(line: &str) -> Option<String> {
    let dq = find_last_quoted(line, '"');
    let sq = find_last_quoted(line, '\'');

    match (dq, sq) {
        (Some((di, ds)), Some((si, ss))) => {
            if di > si {
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

fn find_last_quoted(line: &str, quote_char: char) -> Option<(usize, String)> {
    let end = line.rfind(quote_char)?;
    let start = line[..end].rfind(quote_char)?;
    let content = &line[start + 1..end];
    if content.is_empty() {
        None
    } else {
        Some((end, content.to_string()))
    }
}
