//! Step 1.
//! Discovers all Rust modules under given root folders and parses their
//! public members into a tree structure.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use quote::ToTokens;
use syn::{FnArg, ItemFn, Pat, UseTree, Visibility};

#[derive(Clone, Debug)]
pub struct RustFnArg {
    pub name: String,
    /// Full type path, e.g. `std::collections::HashSet`.
    pub type_name: String,
}

#[derive(Clone, Debug)]
pub struct RustFn {
    /// Fully qualified name from parent module, e.g. `foo::bar::greet`.
    pub name: String,
    pub docs: Option<String>,
    pub args: Vec<RustFnArg>,
    pub return_type_name: String,
}

#[derive(Clone, Debug)]
pub struct RustStructField {
    pub name: String,
    pub docs: Option<String>,
    /// Full type path, e.g. `std::collections::HashSet`.
    pub type_name: String,
}

#[derive(Clone, Debug)]
pub struct RustStruct {
    /// Fully qualified name, e.g. `crate::kitty::Wrap`.
    pub name: String,
    pub docs: Option<String>,
    pub fields: Vec<RustStructField>,
}

#[derive(Clone, Debug)]
pub enum RustMember {
    Module(RustModule),
    Fn(RustFn),
    Struct(RustStruct),
}

#[derive(Clone, Debug)]
pub struct RustModule {
    /// Module name using `::` separators, e.g. `foo::bar`.
    pub name: String,
    /// Absolute path to the module file.
    pub path: String,
    /// Public members (functions, submodules).
    /// `None` if the file could not be read or parsed.
    pub members: Option<Vec<RustMember>>,
}

/// Discovers a crate's public API by parsing its `lib.rs` and walking the module tree.
/// `crate_name` is used as the root module name prefix.
pub fn discover_crate(lib_rs_path: &Path, crate_name: &str) -> Option<RustModule> {
    if !lib_rs_path.exists() {
        return None;
    }
    Some(parse_module_file(lib_rs_path, crate_name))
}

/// Discovers all Rust orphan modules under the given root directory,
/// recursing into subdirectories.
pub fn discover(root: &str) -> Result<Vec<RustModule>, DiscoverError> {
    let root_path =
        fs::canonicalize(root).map_err(|e| DiscoverError::RootNotFound(root.to_string(), e))?;

    let mut result = Vec::new();
    discover_recursive(&root_path, &root_path, &mut result)?;
    Ok(result)
}

fn discover_recursive(
    dir: &Path,
    root: &Path,
    result: &mut Vec<RustModule>,
) -> Result<(), DiscoverError> {
    let entries = fs::read_dir(dir).map_err(|e| DiscoverError::ReadDir(dir.to_path_buf(), e))?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.is_file() && path.extension().is_some_and(|e| e == "rs") {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            if stem == "mod" {
                // avoid discovering module twice
                continue;
            }
            let module_name = build_module_name(&path, root);
            result.push(parse_module_file(&path, &module_name));
        } else if path.is_dir() {
            let mod_file = path.join("mod.rs");
            if mod_file.exists() {
                let module_name = build_module_name(&mod_file, root);
                result.push(parse_module_file(&mod_file, &module_name));
            } else {
                discover_recursive(&path, root, result)?;
            }
        }
    }

    Ok(())
}

fn build_module_name(file_path: &Path, root: &Path) -> String {
    let relative = file_path.strip_prefix(root).unwrap_or(file_path);
    let without_ext = relative.with_extension("");

    let segments: Vec<&str> = without_ext
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    let trimmed = if segments.last() == Some(&"mod") {
        &segments[..segments.len() - 1]
    } else {
        &segments
    };

    trimmed.join("::")
}

#[derive(Debug)]
pub enum DiscoverError {
    RootNotFound(String, std::io::Error),
    ReadDir(PathBuf, std::io::Error),
}

impl std::fmt::Display for DiscoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoverError::RootNotFound(path, e) => {
                write!(f, "root folder '{}' not found: {}", path, e)
            }
            DiscoverError::ReadDir(path, e) => {
                write!(f, "failed to read directory '{}': {}", path.display(), e)
            }
        }
    }
}

fn parse_module_file(file_path: &Path, module_name: &str) -> RustModule {
    let canonical = fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());

    let source = match fs::read_to_string(&canonical) {
        Ok(s) => s,
        Err(_) => {
            return RustModule {
                name: module_name.to_string(),
                path: canonical.to_string_lossy().into_owned(),
                members: None,
            };
        }
    };

    let file = match syn::parse_file(&source) {
        Ok(f) => f,
        Err(_) => {
            return RustModule {
                name: module_name.to_string(),
                path: canonical.to_string_lossy().into_owned(),
                members: None,
            };
        }
    };

    let use_map = collect_uses(&file.items, module_name);
    let parent_dir = canonical.parent().unwrap_or(Path::new("."));
    let members = extract_members(&file.items, module_name, parent_dir, &use_map);

    RustModule {
        name: module_name.to_string(),
        path: canonical.to_string_lossy().into_owned(),
        members: Some(members),
    }
}

fn extract_members(
    items: &[syn::Item],
    parent_name: &str,
    parent_dir: &Path,
    use_map: &UseMap,
) -> Vec<RustMember> {
    // First pass: collect pub fn and pub mod (these are the "real" members)
    let mut members = Vec::new();

    for item in items {
        match item {
            syn::Item::Fn(func) if matches!(func.vis, Visibility::Public(_)) => {
                let func_data = extract_fn(func, use_map);
                members.push(RustMember::Fn(RustFn {
                    name: format!("{}::{}", parent_name, func_data.name),
                    docs: func_data.docs,
                    args: func_data.args,
                    return_type_name: func_data.return_type_name,
                }));
            }
            syn::Item::Struct(item_struct) if matches!(item_struct.vis, Visibility::Public(_)) => {
                let parsed = extract_struct(item_struct, use_map);
                members.push(RustMember::Struct(RustStruct {
                    name: format!("{}::{}", parent_name, parsed.name),
                    docs: parsed.docs,
                    fields: parsed.fields,
                }));
            }
            syn::Item::Mod(item_mod) if matches!(item_mod.vis, Visibility::Public(_)) => {
                let mod_name = item_mod.ident.to_string();
                let full_name = format!("{}::{}", parent_name, mod_name);

                if let Some((_, ref content_items)) = item_mod.content {
                    let inner_use_map = collect_uses(content_items, &full_name);
                    let inner_members =
                        extract_members(content_items, &full_name, parent_dir, &inner_use_map);
                    members.push(RustMember::Module(RustModule {
                        name: full_name,
                        path: String::new(),
                        members: Some(inner_members),
                    }));
                } else {
                    let mod_path = resolve_mod_path(item_mod, parent_dir, &mod_name);
                    if let Some(resolved_path) = mod_path {
                        members.push(RustMember::Module(parse_module_file(
                            &resolved_path,
                            &full_name,
                        )));
                    }
                }
            }
            // Non-pub mods are also parsed so pub use can reference them
            syn::Item::Mod(item_mod) if !matches!(item_mod.vis, Visibility::Public(_)) => {
                let mod_name = item_mod.ident.to_string();
                let full_name = format!("{}::{}", parent_name, mod_name);

                if let Some((_, ref content_items)) = item_mod.content {
                    let inner_use_map = collect_uses(content_items, &full_name);
                    let inner_members =
                        extract_members(content_items, &full_name, parent_dir, &inner_use_map);
                    // Store privately — will be used for pub use resolution but not exposed
                    members.push(RustMember::Module(RustModule {
                        name: full_name,
                        path: String::new(),
                        members: Some(inner_members),
                    }));
                } else {
                    let mod_path = resolve_mod_path(item_mod, parent_dir, &mod_name);
                    if let Some(resolved_path) = mod_path {
                        members.push(RustMember::Module(parse_module_file(
                            &resolved_path,
                            &full_name,
                        )));
                    }
                }
            }
            _ => {}
        }
    }

    // Second pass: resolve `pub use` re-exports by inlining members from submodules
    for item in items {
        if let syn::Item::Use(item_use) = item {
            if matches!(item_use.vis, Visibility::Public(_)) {
                let mut paths = Vec::new();
                flatten_reexport_paths(&item_use.tree, &mut Vec::new(), &mut paths);
                for reexport_path in paths {
                    let inlined = resolve_reexport(&reexport_path, parent_name, &members);
                    for m in inlined {
                        members.push(m);
                    }
                }
            }
        }
    }

    // Filter out private modules (those not declared pub)
    let pub_mod_names: std::collections::HashSet<String> = items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Mod(item_mod) = item {
                if matches!(item_mod.vis, Visibility::Public(_)) {
                    return Some(format!("{}::{}", parent_name, item_mod.ident));
                }
            }
            None
        })
        .collect();

    members.retain(|m| match m {
        RustMember::Module(module) => pub_mod_names.contains(&module.name),
        _ => true,
    });

    members
}

/// Resolves a `pub use` path against the current module's members.
/// Returns inlined copies of the referenced members.
fn resolve_reexport(
    reexport_path: &str,
    parent_name: &str,
    members: &[RustMember],
) -> Vec<RustMember> {
    let segments: Vec<&str> = reexport_path.split("::").collect();
    if segments.is_empty() {
        return vec![];
    }

    // Walk the segments to find the target in the module tree
    let first = segments[0];

    // Handle `self::...` — refers to current module's own members
    let (search_name, rest) = if first == "self" {
        if segments.len() == 1 {
            return vec![];
        }
        let target = format!("{}::{}", parent_name, segments[1]);
        (target, &segments[2..])
    } else {
        let target = format!("{}::{}", parent_name, first);
        (target, &segments[1..])
    };

    // Find the member matching search_name
    let found = members.iter().find(|m| match m {
        RustMember::Module(module) => module.name == search_name,
        RustMember::Fn(f) => f.name == search_name,
        RustMember::Struct(s) => s.name == search_name,
    });

    let Some(found) = found else {
        return vec![];
    };

    if rest.is_empty() {
        // `pub use foo;` — inline the member itself
        return vec![found.clone()];
    }

    // We need to dig deeper into a module
    let RustMember::Module(module) = found else {
        return vec![];
    };

    let Some(ref module_members) = module.members else {
        return vec![];
    };

    let next = rest[0];

    if next == "*" {
        // `pub use foo::*;` — inline all members from the module
        return module_members.clone();
    }

    // `pub use foo::bar` — look for `bar` inside the module
    let deeper_name = format!("{}::{}", module.name, next);
    let deeper_found = module_members.iter().find(|m| match m {
        RustMember::Module(m) => m.name == deeper_name,
        RustMember::Fn(f) => f.name == deeper_name,
        RustMember::Struct(s) => s.name == deeper_name,
    });

    if rest.len() == 1 {
        // Terminal segment
        match deeper_found {
            Some(m) => vec![m.clone()],
            None => vec![],
        }
    } else {
        // More segments to go — recurse
        let remaining = rest[1..].join("::");
        let sub_path = format!("{}::{}", next, remaining);
        resolve_reexport(&sub_path, &module.name, module_members)
    }
}

fn resolve_mod_path(item_mod: &syn::ItemMod, parent_dir: &Path, mod_name: &str) -> Option<PathBuf> {
    // Check for #[path = "..."] attribute
    for attr in &item_mod.attrs {
        if attr.path().is_ident("path") {
            if let syn::Meta::NameValue(nv) = &attr.meta {
                if let syn::Expr::Lit(expr_lit) = &nv.value {
                    if let syn::Lit::Str(s) = &expr_lit.lit {
                        let custom_path = parent_dir.join(s.value());
                        if custom_path.exists() {
                            return Some(custom_path);
                        }
                    }
                }
            }
        }
    }

    let file_path = parent_dir.join(format!("{}.rs", mod_name));
    if file_path.exists() {
        return Some(file_path);
    }

    let dir_path = parent_dir.join(mod_name).join("mod.rs");
    if dir_path.exists() {
        return Some(dir_path);
    }

    None
}

fn flatten_reexport_paths(tree: &UseTree, prefix: &mut Vec<String>, out: &mut Vec<String>) {
    match tree {
        UseTree::Path(use_path) => {
            prefix.push(use_path.ident.to_string());
            flatten_reexport_paths(&use_path.tree, prefix, out);
            prefix.pop();
        }
        UseTree::Name(use_name) => {
            prefix.push(use_name.ident.to_string());
            out.push(prefix.join("::"));
            prefix.pop();
        }
        UseTree::Rename(use_rename) => {
            // `pub use foo as bar` — resolve the original
            prefix.push(use_rename.ident.to_string());
            out.push(prefix.join("::"));
            prefix.pop();
        }
        UseTree::Group(use_group) => {
            for subtree in &use_group.items {
                flatten_reexport_paths(subtree, prefix, out);
            }
        }
        UseTree::Glob(_) => {
            prefix.push("*".to_string());
            out.push(prefix.join("::"));
            prefix.pop();
        }
    }
}

struct ParsedFn {
    name: String,
    docs: Option<String>,
    args: Vec<RustFnArg>,
    return_type_name: String,
}

fn extract_fn(func: &ItemFn, use_map: &UseMap) -> ParsedFn {
    let name = func.sig.ident.to_string();
    let docs = extract_docs(&func.attrs);

    let args = func
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pat_type) = arg {
                let arg_name = match pat_type.pat.as_ref() {
                    Pat::Ident(ident) => ident.ident.to_string(),
                    other => other.to_token_stream().to_string(),
                };
                let raw_tokens = pat_type.ty.to_token_stream();
                let resolved = resolve_type_tokens(raw_tokens, use_map);
                Some(RustFnArg {
                    name: arg_name,
                    type_name: resolved,
                })
            } else {
                None
            }
        })
        .collect();

    let return_type_name = match &func.sig.output {
        syn::ReturnType::Default => String::new(),
        syn::ReturnType::Type(_, ty) => resolve_type_tokens(ty.to_token_stream(), use_map),
    };

    ParsedFn {
        name,
        docs,
        args,
        return_type_name,
    }
}

struct ParsedStruct {
    name: String,
    docs: Option<String>,
    fields: Vec<RustStructField>,
}

fn extract_struct(item_struct: &syn::ItemStruct, use_map: &UseMap) -> ParsedStruct {
    let name = item_struct.ident.to_string();
    let docs = extract_docs(&item_struct.attrs);

    let fields = match &item_struct.fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .filter(|f| matches!(f.vis, Visibility::Public(_)))
            .map(|f| {
                let field_name = f.ident.as_ref().unwrap().to_string();
                let field_docs = extract_docs(&f.attrs);
                let type_name = resolve_type_tokens(f.ty.to_token_stream(), use_map);
                RustStructField {
                    name: field_name,
                    docs: field_docs,
                    type_name,
                }
            })
            .collect(),
        syn::Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .filter(|(_, f)| matches!(f.vis, Visibility::Public(_)))
            .map(|(i, f)| {
                let field_docs = extract_docs(&f.attrs);
                let type_name = resolve_type_tokens(f.ty.to_token_stream(), use_map);
                RustStructField {
                    name: i.to_string(),
                    docs: field_docs,
                    type_name,
                }
            })
            .collect(),
        syn::Fields::Unit => vec![],
    };

    ParsedStruct { name, docs, fields }
}

type UseMap = HashMap<String, String>;

fn collect_uses(items: &[syn::Item], module_name: &str) -> UseMap {
    let mut map = UseMap::new();
    for item in items {
        if let syn::Item::Use(item_use) = item {
            flatten_use_tree(&item_use.tree, &mut Vec::new(), &mut map);
        }
    }
    // Also map locally declared types (structs, enums, type aliases, unions)
    for item in items {
        let type_name = match item {
            syn::Item::Struct(s) => Some(s.ident.to_string()),
            syn::Item::Enum(e) => Some(e.ident.to_string()),
            syn::Item::Type(t) => Some(t.ident.to_string()),
            syn::Item::Union(u) => Some(u.ident.to_string()),
            _ => None,
        };
        if let Some(name) = type_name {
            let full_path = format!("crate::{}::{}", module_name, name);
            map.entry(name).or_insert(full_path);
        }
    }
    map
}

fn flatten_use_tree(tree: &UseTree, prefix: &mut Vec<String>, map: &mut UseMap) {
    match tree {
        UseTree::Path(use_path) => {
            prefix.push(use_path.ident.to_string());
            flatten_use_tree(&use_path.tree, prefix, map);
            prefix.pop();
        }
        UseTree::Name(use_name) => {
            let leaf = use_name.ident.to_string();
            prefix.push(leaf.clone());
            map.insert(leaf, prefix.join("::"));
            prefix.pop();
        }
        UseTree::Rename(use_rename) => {
            let alias = use_rename.rename.to_string();
            prefix.push(use_rename.ident.to_string());
            map.insert(alias, prefix.join("::"));
            prefix.pop();
        }
        UseTree::Group(use_group) => {
            for subtree in &use_group.items {
                flatten_use_tree(subtree, prefix, map);
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn resolve_type_tokens(tokens: TokenStream, use_map: &UseMap) -> String {
    let resolved: TokenStream = tokens
        .into_iter()
        .map(|tt| match tt {
            TokenTree::Ident(ref ident) => {
                let name = ident.to_string();
                if let Some(full_path) = use_map.get(&name) {
                    let path_tokens: TokenStream = full_path
                        .split("::")
                        .enumerate()
                        .flat_map(|(i, segment)| {
                            let mut toks: Vec<TokenTree> = Vec::new();
                            if i > 0 {
                                toks.push(TokenTree::Punct(proc_macro2::Punct::new(
                                    ':',
                                    proc_macro2::Spacing::Joint,
                                )));
                                toks.push(TokenTree::Punct(proc_macro2::Punct::new(
                                    ':',
                                    proc_macro2::Spacing::Alone,
                                )));
                            }
                            toks.push(TokenTree::Ident(Ident::new(segment, Span::call_site())));
                            toks
                        })
                        .collect();
                    return path_tokens;
                }
                TokenStream::from(tt)
            }
            TokenTree::Group(group) => {
                let inner = resolve_type_tokens(group.stream(), use_map);
                let resolved_inner: TokenStream = inner.parse().unwrap_or_default();
                let new_group = proc_macro2::Group::new(group.delimiter(), resolved_inner);
                TokenStream::from(TokenTree::Group(new_group))
            }
            _ => TokenStream::from(tt),
        })
        .collect();

    resolved.to_string()
}

fn extract_docs(attrs: &[syn::Attribute]) -> Option<String> {
    let lines: Vec<String> = attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }
            if let syn::Meta::NameValue(nv) = &attr.meta {
                if let syn::Expr::Lit(expr_lit) = &nv.value {
                    if let syn::Lit::Str(s) = &expr_lit.lit {
                        return Some(s.value());
                    }
                }
            }
            None
        })
        .collect();

    if lines.is_empty() {
        return None;
    }

    let trimmed: Vec<&str> = lines
        .iter()
        .map(|l| l.strip_prefix(' ').unwrap_or(l))
        .collect();
    Some(trimmed.join("\n"))
}
