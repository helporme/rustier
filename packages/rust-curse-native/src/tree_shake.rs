//! Walks the IR starting from items imported by JS/TS code and returns the
//! set of `ir::Id`s that should be exposed in target glue (napi/wasm) and the
//! rust-analyzer fallback section.
//!
//! TS ambient declarations are *not* gated by this set; they always reflect
//! the full IR so editor intellisense surfaces every public item.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use glob::glob;

use crate::common::mangle_module_name;
use crate::ir::{Ctor, Id, Item, StructShape, Type, IR};
use crate::orphans;
use crate::FileFilter;

#[derive(Debug)]
pub struct TreeShakeOpts<'a> {
    /// Absolute path of the project root.
    pub project_root_abs_path: &'a Path,
    /// Source root, relative to `project_root_abs_path`. JS specifiers
    /// like `'../src/greet.rs'` resolve to a `.rs` file under this root.
    pub src_rel_path: &'a Path,
    /// Filters selecting which JS/TS files are scanned for imports.
    pub js_filter: &'a FileFilter,
}

#[derive(Debug, thiserror::Error)]
pub enum TreeShakeError {
    #[error("root folder '{}' not found", abs_path.display())]
    RootNotFound {
        abs_path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid glob pattern '{pattern}'")]
    InvalidGlob {
        pattern: String,
        #[source]
        source: glob::PatternError,
    },
    #[error("failed to read file '{}'", abs_path.display())]
    ReadFile {
        abs_path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "default import of '{}' is not supported; rust modules have no default export, \
         use named imports like `import {{ name }} from '...'`",
        rel_rs_path.display()
    )]
    DefaultImport { rel_rs_path: PathBuf },
    #[error("import from '{}' did not resolve to any IR module", rel_rs_path.display())]
    UnresolvedFile { rel_rs_path: PathBuf },
    #[error("import '{name}' from '{}' did not resolve to any IR item", rel_rs_path.display())]
    UnresolvedImport { rel_rs_path: PathBuf, name: String },
}

/// Compute the set of IR ids reachable from JS/TS imports.
pub fn shake(ir: &IR, opts: TreeShakeOpts<'_>) -> Result<HashSet<Id>, TreeShakeError> {
    let abs_project_root = fs::canonicalize(opts.project_root_abs_path).map_err(|source| {
        TreeShakeError::RootNotFound {
            abs_path: opts.project_root_abs_path.to_path_buf(),
            source,
        }
    })?;

    let abs_src_root = abs_project_root.join(opts.src_rel_path);
    let abs_src_root =
        fs::canonicalize(&abs_src_root).map_err(|source| TreeShakeError::RootNotFound {
            abs_path: abs_src_root,
            source,
        })?;

    let mut buckets: BTreeMap<PathBuf, Bucket> = BTreeMap::new();
    for import in scan_js_imports(&abs_src_root, opts.js_filter)? {
        if import.members.iter().any(|m| m == "default") {
            return Err(TreeShakeError::DefaultImport {
                rel_rs_path: opts.src_rel_path.join(&import.rel_rs_path),
            });
        }
        let entry = buckets.entry(import.rel_rs_path).or_default();
        for m in import.members {
            if m == "*" {
                entry.star = true;
            } else {
                entry.members.insert(m);
            }
        }
    }

    let mut seeds: Vec<Id> = Vec::new();
    for (rel_within_src_root, bucket) in buckets {
        let mod_name = orphans::gen_module_name(&rel_within_src_root);
        let rel_rs_path = opts.src_rel_path.join(&rel_within_src_root);
        let Some(mod_id) = find_module(ir, &mod_name) else {
            return Err(TreeShakeError::UnresolvedFile { rel_rs_path });
        };
        let exports = &ir.get_mod_unchecked(mod_id).exports;
        if bucket.star {
            for export in exports {
                seeds.push(export.target);
            }
        } else {
            for name in &bucket.members {
                let Some(export) = exports.iter().find(|e| &e.name == name) else {
                    return Err(TreeShakeError::UnresolvedImport {
                        rel_rs_path,
                        name: name.clone(),
                    });
                };
                seeds.push(export.target);
            }
        }
    }

    let mut visited: HashSet<Id> = HashSet::new();
    let mut stack = seeds;
    while let Some(id) = stack.pop() {
        if !visited.insert(id) {
            continue;
        }
        match ir.get_unchecked(id) {
            Item::Fn(def) => {
                for param in &def.params {
                    walk_type(&param.ty, &mut stack);
                }
                walk_type(&def.return_type, &mut stack);
            }
            Item::Struct(def) => {
                if let Ctor::Explicit(fid) = def.ctor {
                    stack.push(fid);
                }
                match &def.shape {
                    StructShape::Unit => {}
                    StructShape::Tuple(ts) => {
                        for ty in ts {
                            walk_type(ty, &mut stack);
                        }
                    }
                    StructShape::Struct(fs) => {
                        for f in fs {
                            walk_type(&f.ty, &mut stack);
                        }
                    }
                }
            }
            Item::Mod(_) => {}
        }
    }

    Ok(visited)
}

fn walk_type(ty: &Type, stack: &mut Vec<Id>) {
    if let Type::Struct(sid) = ty {
        stack.push(*sid);
    }
}

fn find_module(ir: &IR, mod_name: &str) -> Option<Id> {
    let mangled = mangle_module_name(mod_name);
    let expected: Vec<&str> = vec![ir.crate_name.as_str(), mangled.as_str()];
    for (i, item) in ir.items.iter().enumerate() {
        let Item::Mod(def) = item else { continue };
        let segments: Vec<&str> = def.path.segments.iter().map(String::as_str).collect();
        if segments == expected {
            return Some(Id(i as u32));
        }
    }
    None
}

#[derive(Default)]
struct Bucket {
    star: bool,
    members: BTreeSet<String>,
}

/// One ESM import of an `.rs` module found in a single JS/TS file.
struct RawImport {
    /// Path of the imported `.rs` file relative to `abs_module_dir`.
    rel_rs_path: PathBuf,
    /// Imported member names. May contain `"*"` for star imports and
    /// `"default"` for default imports.
    members: Vec<String>,
}

fn scan_js_imports(
    abs_module_dir: &Path,
    filter: &FileFilter,
) -> Result<Vec<RawImport>, TreeShakeError> {
    let include = filter.include.as_deref().unwrap_or(&[]);
    let exclude = filter.exclude.as_deref().unwrap_or(&[]);

    let excluded_paths = expand_paths(exclude)?;
    let mut result = Vec::new();

    for pattern in include {
        let entries = glob(pattern).map_err(|source| TreeShakeError::InvalidGlob {
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

            let source =
                fs::read_to_string(&abs_path).map_err(|source| TreeShakeError::ReadFile {
                    abs_path: abs_path.clone(),
                    source,
                })?;

            let file_dir = abs_path.parent().unwrap_or(Path::new("."));
            result.extend(extract_imports(&source, file_dir, abs_module_dir));
        }
    }

    Ok(result)
}

fn expand_paths(patterns: &[String]) -> Result<HashSet<PathBuf>, TreeShakeError> {
    let mut paths = HashSet::new();
    for pattern in patterns {
        let entries = glob(pattern).map_err(|source| TreeShakeError::InvalidGlob {
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

fn extract_imports(source: &str, file_dir: &Path, abs_module_dir: &Path) -> Vec<RawImport> {
    let cleaned = strip_comments(source);
    let mut results = Vec::new();

    for stmt in collect_import_statements(&cleaned) {
        if let Some(import) = parse_import_statement(&stmt, file_dir, abs_module_dir) {
            results.push(import);
        }
    }

    results
}

fn strip_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut result = String::with_capacity(source.len());
    let mut i = 0;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < bytes.len() {
                i += 2;
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    result
}

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

fn parse_import_statement(stmt: &str, file_dir: &Path, abs_module_dir: &Path) -> Option<RawImport> {
    let from_idx = stmt.rfind(" from ")?;
    let keyword_end = 6;
    let members_part = &stmt[keyword_end..from_idx];

    if members_part.trim_start().starts_with("type") {
        let after_type = members_part.trim_start().strip_prefix("type")?;
        let first_non_type = after_type.chars().next().unwrap_or(' ');
        if first_non_type == ' ' || first_non_type == '{' {
            return None;
        }
    }

    let after_from = &stmt[from_idx + 6..];
    let specifier = extract_first_string_literal(after_from)?;

    let rel_rs_path = resolve_specifier(&specifier, file_dir, abs_module_dir)?;

    let members = parse_members(members_part);
    if members.is_empty() {
        return None;
    }

    Some(RawImport {
        rel_rs_path,
        members,
    })
}

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

fn resolve_specifier(specifier: &str, file_dir: &Path, abs_module_dir: &Path) -> Option<PathBuf> {
    if !specifier.ends_with(".rs") {
        return None;
    }
    let candidate = file_dir.join(specifier);
    try_resolve(&candidate, abs_module_dir)
}

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
            .filter_map(|s| {
                let first = s.split_whitespace().next()?;
                if first == "type" {
                    None
                } else {
                    Some(first.to_string())
                }
            })
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

/// Return the segments-joined `crate::path::to::item` string for a given Id.
/// Used by tree_shake tests to render id sets in a stable, human-readable form.
pub fn item_path(ir: &IR, id: Id) -> String {
    let segs = match ir.get_unchecked(id) {
        Item::Fn(def) => &def.path.segments,
        Item::Struct(def) => &def.path.segments,
        Item::Mod(def) => &def.path.segments,
    };
    segs.join("::")
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
    fn parse_inline_type_member_dropped() {
        assert_eq!(parse_members(" { type Foo } "), Vec::<String>::new());
        assert_eq!(parse_members(" { type Foo as F } "), Vec::<String>::new());
        assert_eq!(
            parse_members(" { type Foo, type Bar } "),
            Vec::<String>::new()
        );
    }

    #[test]
    fn parse_mixed_type_and_value_members() {
        assert_eq!(parse_members(" { Foo, type Bar } "), vec!["Foo"]);
        assert_eq!(parse_members(" { type Bar, Foo } "), vec!["Foo"]);
    }

    #[test]
    fn parse_identifier_starting_with_type_kept() {
        assert_eq!(parse_members(" { typeValue } "), vec!["typeValue"]);
    }
}
