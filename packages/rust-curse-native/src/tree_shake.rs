//! Walks the ASG starting from items imported by JS/TS code and returns the
//! set of `asg::Id`s that should be exposed in target glue (napi/wasm) and the
//! rust-analyzer fallback section.
//!
//! TS ambient declarations are *not* gated by this set; they always reflect
//! the full ASG so editor intellisense surfaces every public item.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use glob::glob;
use tracing::*;

use crate::codegen::asg::{Asg, AsgId, Item, StructShape, Type};
use crate::common::FileFilter;
use crate::orphans;

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
}

/// Compute the set of ASG ids reachable from JS/TS imports. JS imports that
/// reference a non-existent `.rs` module, a name not exported by the
/// targeted module, or use a default-import syntax are skipped with debug logs.
pub fn shake(asg: &Asg, opts: TreeShakeOpts<'_>) -> Result<HashSet<AsgId>, TreeShakeError> {
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
        // Default imports are unsupported (rust modules have no default
        // export), so the import cannot contribute tree-shake seeds.
        if import.members.iter().any(|m| m == "default") {
            debug!(
                js = %import.source_abs_path.display(),
                rust = %import.rel_rs_path.display(),
                member = "default",
                "ignored default import while collecting reachability seeds"
            );
            continue;
        }
        let entry = buckets.entry(import.rel_rs_path).or_default();
        entry
            .source_abs_paths
            .insert(import.source_abs_path.clone());
        for m in import.members {
            if m == "*" {
                entry.star = true;
            } else {
                entry
                    .members
                    .entry(m)
                    .or_default()
                    .insert(import.source_abs_path.clone());
            }
        }
    }

    let mut seeds: Vec<AsgId> = Vec::new();
    for (rel_within_src_root, bucket) in buckets {
        let mod_name = orphans::gen_module_name(&rel_within_src_root);
        let Some(mod_id) = find_module(asg, &mod_name) else {
            let members: Vec<&str> = bucket.members.keys().map(String::as_str).collect();
            debug!(
                rust = %rel_within_src_root.display(),
                module = %mod_name,
                star = bucket.star,
                members = ?members,
                js = ?bucket.source_abs_paths,
                "ignored import for rust module missing from ASG"
            );
            continue;
        };
        let uses = &asg.get_mod_unchecked(mod_id).uses;
        if bucket.star {
            for use_ in uses {
                seeds.push(use_.target);
            }
        } else {
            for (name, source_abs_paths) in &bucket.members {
                let Some(use_) = uses.iter().find(|u| &u.name == name) else {
                    debug!(
                        rust = %rel_within_src_root.display(),
                        module = %mod_name,
                        member = %name,
                        js = ?source_abs_paths,
                        "ignored import for missing named export"
                    );
                    continue;
                };
                seeds.push(use_.target);
            }
        }
    }

    debug!(seeds = seeds.len(), "collected reachability seeds");

    let mut visited: HashSet<AsgId> = HashSet::new();
    let mut stack = seeds;
    while let Some(id) = stack.pop() {
        if !visited.insert(id) {
            continue;
        }
        let item = asg.get_unchecked(id);
        let (kind, path) = item_desc(item);
        trace!(asg_id = id.stable, kind, %path, "visiting reachable item");
        match item {
            Item::Fn(def) => {
                for param in &def.params {
                    walk_type(&param.ty, &mut stack);
                }
                walk_type(&def.return_type, &mut stack);
            }
            Item::Struct(def) => {
                for impl_id in &def.impls {
                    let impl_ = asg.get_impl_unchecked(*impl_id);
                    if impl_.trait_.is_some() {
                        continue;
                    }
                    for fid in &impl_.fns {
                        stack.push(*fid);
                    }
                }
                walk_shape(&def.shape, &mut stack);
            }
            Item::Enum(def) => {
                for impl_id in &def.impls {
                    let impl_ = asg.get_impl_unchecked(*impl_id);
                    if impl_.trait_.is_some() {
                        continue;
                    }
                    for fid in &impl_.fns {
                        stack.push(*fid);
                    }
                }
                for variant in &def.variants {
                    walk_shape(&variant.shape, &mut stack);
                }
            }
            Item::Mod(_) | Item::Impl(_) | Item::Trait(_) => {}
        }
    }

    Ok(visited)
}

fn walk_shape(shape: &StructShape, stack: &mut Vec<AsgId>) {
    match shape {
        StructShape::Unit => {}
        StructShape::Tuple(ts) => {
            for ty in ts {
                walk_type(ty, stack);
            }
        }
        StructShape::Struct(fs) => {
            for f in fs {
                walk_type(&f.ty, stack);
            }
        }
    }
}

fn walk_type(ty: &Type, stack: &mut Vec<AsgId>) {
    if let Type::Struct(sid) | Type::Enum(sid) = ty {
        stack.push(*sid);
    }
}

/// A short `(kind, path)` label for an item, used in trace logging. Impls have
/// no path of their own, so they are described by the struct they are for.
fn item_desc(item: &Item) -> (&'static str, String) {
    match item {
        Item::Struct(def) => ("struct", def.path.path.join("::")),
        Item::Enum(def) => ("enum", def.path.path.join("::")),
        Item::Fn(def) => ("fn", def.path.path.join("::")),
        Item::Mod(def) => ("mod", def.path.path.join("::")),
        Item::Trait(def) => ("trait", def.path.path.join("::")),
        Item::Impl(def) => ("impl", format!("for #{}", def.for_.index)),
    }
}

fn find_module(asg: &Asg, mod_name: &str) -> Option<AsgId> {
    // Orphan modules are flattened into one ident by replacing `::` with
    // `__` (see the generated `pub mod nested__deep;`). Underscores in the
    // source name are preserved. Using `mangle_module_name` here was a bug:
    // it additionally rewrites `_` -> `0_`, so a module whose name contains
    // an underscore (e.g. `data_enums`) never matched and was silently
    // dropped from tree-shaking, leaving its napi/wasm glue unexposed.
    let flattened = mod_name.replace("::", "__");
    let expected: Vec<&str> = vec!["crate", flattened.as_str()];
    for item in &asg.items {
        let Item::Mod(def) = item else { continue };
        let segments: Vec<&str> = def.path.path.iter().map(String::as_str).collect();
        if segments == expected {
            return Some(def.id);
        }
    }
    None
}

#[derive(Default)]
struct Bucket {
    star: bool,
    members: BTreeMap<String, BTreeSet<PathBuf>>,
    source_abs_paths: BTreeSet<PathBuf>,
}

/// One ESM import of an `.rs` module found in a single JS/TS file.
struct RawImport {
    /// Absolute path of the JS/TS file containing this import.
    source_abs_path: PathBuf,
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

fn extract_imports(
    source: &str,
    source_abs_path: &Path,
    file_dir: &Path,
    abs_module_dir: &Path,
) -> Vec<RawImport> {
    let cleaned = strip_comments(source);
    let mut results = Vec::new();

    for stmt in collect_import_statements(&cleaned) {
        if let Some(import) =
            parse_import_statement(&stmt, source_abs_path, file_dir, abs_module_dir)
        {
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

fn parse_import_statement(
    stmt: &str,
    source_abs_path: &Path,
    file_dir: &Path,
    abs_module_dir: &Path,
) -> Option<RawImport> {
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
        source_abs_path: source_abs_path.to_path_buf(),
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
pub fn item_path(asg: &Asg, id: AsgId) -> String {
    let segs = match asg.get_unchecked(id) {
        Item::Fn(def) => &def.path.path,
        Item::Struct(def) => &def.path.path,
        Item::Enum(def) => &def.path.path,
        Item::Mod(def) => &def.path.path,
        Item::Trait(def) => &def.path.path,
        Item::Impl(_) => return "<impl>".to_string(),
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
