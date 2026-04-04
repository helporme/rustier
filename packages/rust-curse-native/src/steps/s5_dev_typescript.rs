//! Step 5 for dev target.
//! Generates per-module `.d.ts` files mirroring the source directory structure,
//! plus an `ambient.d.ts` declaring virtual modules for cross-module and
//! external crate type imports.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;
use std::fs;
use std::path::Path;

use crate::steps::s1_discovery::{RustMember, RustModule, RustStruct};

// ---------------------------------------------------------------------------
// Type registry
// ---------------------------------------------------------------------------

/// Maps fully-qualified Rust type path (e.g. `crate::kitty::Wrap`) to its struct.
type TypeRegistry = HashMap<String, RustStruct>;

fn build_type_registry(
    local_modules: &[RustModule],
    dep_modules: &HashMap<String, RustModule>,
) -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    for module in local_modules {
        collect_structs_from_module(module, &mut registry, true);
    }
    for module in dep_modules.values() {
        collect_structs_from_module(module, &mut registry, false);
    }
    registry
}

fn collect_structs_from_module(module: &RustModule, registry: &mut TypeRegistry, is_local: bool) {
    let members = match &module.members {
        Some(m) => m,
        None => return,
    };
    for member in members {
        match member {
            RustMember::Struct(s) => {
                registry.insert(s.name.clone(), s.clone());
                // Local structs are referenced as "crate::module::Type" in type_name strings
                if is_local {
                    registry.insert(format!("crate::{}", s.name), s.clone());
                }
            }
            RustMember::Module(sub) => {
                collect_structs_from_module(sub, registry, is_local);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Rust type → TS type
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum TsType {
    Primitive(String),
    Array(Box<TsType>),
    Tuple(Vec<TsType>),
    Nullable(Box<TsType>),
    Record(Box<TsType>, Box<TsType>),
    /// A reference to a known struct type.
    StructRef {
        short: String,
        full_path: String,
    },
    Unknown,
}

fn rust_type_to_ts(type_name: &str, registry: &TypeRegistry) -> TsType {
    let tokens = tokenize_type(type_name);
    parse_type_tokens(&tokens, registry)
}

/// Simple tokenizer: splits on whitespace but keeps `<`, `>`, `(`, `)`, `,`, `&` as individual tokens.
fn tokenize_type(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in s.chars() {
        match ch {
            '<' | '>' | '(' | ')' | ',' | '&' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                tokens.push(ch.to_string());
            }
            ' ' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn parse_type_tokens(tokens: &[String], registry: &TypeRegistry) -> TsType {
    let mut pos = 0;
    parse_type_expr(tokens, &mut pos, registry)
}

fn parse_type_expr(tokens: &[String], pos: &mut usize, registry: &TypeRegistry) -> TsType {
    if *pos >= tokens.len() {
        return TsType::Unknown;
    }

    let token = &tokens[*pos];

    // Tuple: (A, B, ...)
    if token == "(" {
        *pos += 1;
        // Check for unit tuple ()
        if *pos < tokens.len() && tokens[*pos] == ")" {
            *pos += 1;
            return TsType::Primitive("void".into());
        }
        let mut elems = Vec::new();
        loop {
            if *pos >= tokens.len() {
                break;
            }
            if tokens[*pos] == ")" {
                *pos += 1;
                break;
            }
            if tokens[*pos] == "," {
                *pos += 1;
                continue;
            }
            elems.push(parse_type_expr(tokens, pos, registry));
        }
        if elems.len() == 1 {
            return elems.into_iter().next().unwrap();
        }
        return TsType::Tuple(elems);
    }

    // Reference: &T or &'a T
    if token == "&" {
        *pos += 1;
        // Skip lifetime if present
        if *pos < tokens.len() && tokens[*pos].starts_with('\'') {
            *pos += 1;
        }
        // &str -> string
        if *pos < tokens.len() && tokens[*pos] == "str" {
            *pos += 1;
            return TsType::Primitive("string".into());
        }
        // &[T] would show as `& [ T ]` but in our tokenstream it's unlikely;
        // just parse the inner type
        return parse_type_expr(tokens, pos, registry);
    }

    // Path or identifier: consume segments joined by `::`
    let mut path_segments = vec![token.clone()];
    *pos += 1;
    while *pos + 1 < tokens.len() && tokens[*pos] == "::" {
        *pos += 1; // skip ::
        if *pos < tokens.len() {
            path_segments.push(tokens[*pos].clone());
            *pos += 1;
        }
    }

    let full_path = path_segments.join("::");
    let leaf = path_segments.last().unwrap().clone();

    // Check for generics: `Name < ... >`
    let has_generics = *pos < tokens.len() && tokens[*pos] == "<";
    let generic_args = if has_generics {
        *pos += 1; // skip <
        let mut args = Vec::new();
        let mut depth = 1;
        loop {
            if *pos >= tokens.len() {
                break;
            }
            if tokens[*pos] == ">" {
                depth -= 1;
                if depth == 0 {
                    *pos += 1;
                    break;
                }
            }
            if tokens[*pos] == "<" {
                depth += 1;
            }
            if tokens[*pos] == "," && depth == 1 {
                *pos += 1;
                continue;
            }
            // Only parse at depth 1
            if depth == 1 {
                args.push(parse_type_expr(tokens, pos, registry));
            } else {
                *pos += 1;
            }
        }
        args
    } else {
        vec![]
    };

    // Resolve the type
    resolve_type(&full_path, &leaf, &generic_args, registry)
}

fn resolve_type(
    full_path: &str,
    leaf: &str,
    generic_args: &[TsType],
    registry: &TypeRegistry,
) -> TsType {
    // Primitives
    match leaf {
        "String" if !full_path.contains("::") => return TsType::Primitive("string".into()),
        "str" => return TsType::Primitive("string".into()),
        "bool" => return TsType::Primitive("boolean".into()),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
        | "u128" | "usize" | "f32" | "f64" => {
            return TsType::Primitive("number".into());
        }
        _ => {}
    }

    // Known generic wrappers
    match leaf {
        "Vec" if generic_args.len() == 1 => {
            return TsType::Array(Box::new(generic_args[0].clone()));
        }
        "Option" if generic_args.len() == 1 => {
            return TsType::Nullable(Box::new(generic_args[0].clone()));
        }
        "HashMap" | "BTreeMap" if generic_args.len() == 2 => {
            return TsType::Record(
                Box::new(generic_args[0].clone()),
                Box::new(generic_args[1].clone()),
            );
        }
        _ => {}
    }

    // Look up in registry
    if registry.contains_key(full_path) {
        let short = leaf.to_string();
        return TsType::StructRef {
            short,
            full_path: full_path.to_string(),
        };
    }

    TsType::Unknown
}

// ---------------------------------------------------------------------------
// TsType → string
// ---------------------------------------------------------------------------

fn ts_type_to_string(ty: &TsType, registry: &TypeRegistry) -> String {
    match ty {
        TsType::Primitive(s) => s.clone(),
        TsType::Array(inner) => {
            let inner_str = ts_type_to_string(inner, registry);
            // Wrap unions in parens: (T | null)[]
            if matches!(inner.as_ref(), TsType::Nullable(_)) {
                format!("({})[]", inner_str)
            } else {
                format!("{}[]", inner_str)
            }
        }
        TsType::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(|e| ts_type_to_string(e, registry)).collect();
            format!("[{}]", parts.join(", "))
        }
        TsType::Nullable(inner) => {
            format!("{} | null", ts_type_to_string(inner, registry))
        }
        TsType::Record(k, v) => {
            format!(
                "Record<{}, {}>",
                ts_type_to_string(k, registry),
                ts_type_to_string(v, registry)
            )
        }
        TsType::StructRef { short, .. } => short.clone(),
        TsType::Unknown => "unknown".into(),
    }
}

fn struct_to_ts_type(s: &RustStruct, registry: &TypeRegistry) -> String {
    if s.fields.is_empty() {
        return "null".into();
    }

    let is_tuple = s.fields.first().is_some_and(|f| f.name == "0");

    if is_tuple {
        if s.fields.len() == 1 {
            // Newtype: unwrap to inner type
            let ts = rust_type_to_ts(&s.fields[0].type_name, registry);
            return ts_type_to_string(&ts, registry);
        }
        // Multi-field tuple
        let parts: Vec<String> = s
            .fields
            .iter()
            .map(|f| {
                let ts = rust_type_to_ts(&f.type_name, registry);
                ts_type_to_string(&ts, registry)
            })
            .collect();
        return format!("[{}]", parts.join(", "));
    }

    // Named fields → object type
    let mut out = String::from("{\n");
    for field in &s.fields {
        if let Some(ref docs) = field.docs {
            writeln!(out, "    /**").unwrap();
            for line in docs.lines() {
                writeln!(out, "     * {}", line).unwrap();
            }
            writeln!(out, "     */").unwrap();
        }
        let ts = rust_type_to_ts(&field.type_name, registry);
        writeln!(out, "    {}: {};", field.name, ts_type_to_string(&ts, registry)).unwrap();
    }
    write!(out, "  }}").unwrap();
    out
}

// ---------------------------------------------------------------------------
// Collect type references for imports
// ---------------------------------------------------------------------------

fn collect_type_refs(ty: &TsType, refs: &mut Vec<(String, String)>) {
    match ty {
        TsType::StructRef { short, full_path } => {
            refs.push((short.clone(), full_path.clone()));
        }
        TsType::Array(inner) | TsType::Nullable(inner) => collect_type_refs(inner, refs),
        TsType::Tuple(elems) => {
            for e in elems {
                collect_type_refs(e, refs);
            }
        }
        TsType::Record(k, v) => {
            collect_type_refs(k, refs);
            collect_type_refs(v, refs);
        }
        _ => {}
    }
}

/// Collect type refs from a struct's fields (for when structs are written inline).
fn collect_struct_field_refs(s: &RustStruct, registry: &TypeRegistry) -> Vec<(String, String)> {
    let mut refs = Vec::new();
    for field in &s.fields {
        let ts = rust_type_to_ts(&field.type_name, registry);
        collect_type_refs(&ts, &mut refs);
    }
    refs
}

/// Determines which `$rust/` module a full Rust type path should be imported from.
/// Returns `None` if it's a local type (defined in `current_module`).
///
/// Uses full module path: `crate::kitty::Wrap` → `$rust/crate_name/kitty`,
/// `serde_json::value::Value` → `$rust/serde_json/value`.
fn import_module_for(full_path: &str, current_module: &str, crate_name: &str) -> Option<String> {
    if full_path.starts_with("crate::") {
        // Check if it belongs to the current module
        let type_module = full_path
            .rsplit_once("::")
            .map(|(prefix, _)| prefix)
            .unwrap_or(full_path);
        let current_crate_module = format!("crate::{}", current_module);
        if type_module == current_crate_module {
            return None;
        }
        // crate::kitty::Wrap → module path is "kitty"
        let without_crate = full_path.strip_prefix("crate::").unwrap();
        let module_path = without_crate
            .rsplit_once("::")
            .map(|(prefix, _)| prefix)
            .unwrap_or(without_crate);
        Some(format!("$rust/{}/{}", crate_name, module_path.replace("::", "/")))
    } else {
        // External crate: serde_json::value::Value → $rust/serde_json/value
        let module_path = full_path
            .rsplit_once("::")
            .map(|(prefix, _)| prefix)
            .unwrap_or(full_path);
        Some(format!("$rust/{}", module_path.replace("::", "/")))
    }
}

// ---------------------------------------------------------------------------
// Per-module .d.ts generation
// ---------------------------------------------------------------------------

fn generate_dts_content(
    module: &RustModule,
    registry: &TypeRegistry,
    crate_name: &str,
) -> String {
    let members = match &module.members {
        Some(m) => m,
        None => return String::from("// Auto-generated by rust-curse.\n"),
    };

    // Collect all type refs from function signatures and local struct fields
    let mut all_refs: Vec<(String, String)> = Vec::new();
    let mut local_structs: Vec<&RustStruct> = Vec::new();

    collect_member_refs(members, registry, &module.name, &mut all_refs, &mut local_structs);

    // Also collect refs from local struct fields
    for s in &local_structs {
        let field_refs = collect_struct_field_refs(s, registry);
        all_refs.extend(field_refs);
    }

    // Group imports by module, excluding local types
    let mut imports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (short, full_path) in &all_refs {
        if let Some(module_path) = import_module_for(full_path, &module.name, crate_name) {
            imports
                .entry(module_path)
                .or_default()
                .insert(short.clone());
        }
    }

    let mut out = String::from("// Auto-generated by rust-curse.\n");

    // Emit imports
    for (module_path, names) in &imports {
        let names_str: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        writeln!(out, "import type {{ {} }} from '{}';", names_str.join(", "), module_path).unwrap();
    }
    if !imports.is_empty() {
        writeln!(out).unwrap();
    }

    // Emit members
    write_members(&mut out, members, registry);

    out
}

fn collect_member_refs<'a>(
    members: &'a [RustMember],
    registry: &TypeRegistry,
    _module_name: &str,
    refs: &mut Vec<(String, String)>,
    local_structs: &mut Vec<&'a RustStruct>,
) {
    for member in members {
        match member {
            RustMember::Fn(func) => {
                for arg in &func.args {
                    let ts = rust_type_to_ts(&arg.type_name, registry);
                    collect_type_refs(&ts, refs);
                }
                if !func.return_type_name.is_empty() {
                    let ts = rust_type_to_ts(&func.return_type_name, registry);
                    collect_type_refs(&ts, refs);
                }
            }
            RustMember::Struct(s) => {
                local_structs.push(s);
            }
            RustMember::Module(sub) => {
                if let Some(ref sub_members) = sub.members {
                    collect_member_refs(sub_members, registry, _module_name, refs, local_structs);
                }
            }
        }
    }
}

fn write_members(out: &mut String, members: &[RustMember], registry: &TypeRegistry) {
    for member in members {
        match member {
            RustMember::Fn(func) => {
                let short_name = func.name.rsplit("::").next().unwrap_or(&func.name);

                if let Some(ref docs) = func.docs {
                    writeln!(out, "/**").unwrap();
                    for line in docs.lines() {
                        writeln!(out, " * {}", line).unwrap();
                    }
                    writeln!(out, " */").unwrap();
                }

                let args: Vec<String> = func
                    .args
                    .iter()
                    .map(|a| {
                        let ts = rust_type_to_ts(&a.type_name, registry);
                        format!("{}: {}", a.name, ts_type_to_string(&ts, registry))
                    })
                    .collect();

                let ret = if func.return_type_name.is_empty() {
                    "void".into()
                } else {
                    let ts = rust_type_to_ts(&func.return_type_name, registry);
                    ts_type_to_string(&ts, registry)
                };

                writeln!(out, "export function {}({}): {};", short_name, args.join(", "), ret)
                    .unwrap();
            }
            RustMember::Struct(s) => {
                let short_name = s.name.rsplit("::").next().unwrap_or(&s.name);

                if let Some(ref docs) = s.docs {
                    writeln!(out, "/**").unwrap();
                    for line in docs.lines() {
                        writeln!(out, " * {}", line).unwrap();
                    }
                    writeln!(out, " */").unwrap();
                }

                let ts_body = struct_to_ts_type(s, registry);
                writeln!(out, "export type {} = {};", short_name, ts_body).unwrap();
            }
            RustMember::Module(sub) => {
                let short_name = sub.name.rsplit("::").next().unwrap_or(&sub.name);
                if let Some(ref sub_members) = sub.members {
                    writeln!(out, "export namespace {} {{", short_name).unwrap();
                    write_members(out, sub_members, registry);
                    writeln!(out, "}}").unwrap();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ambient.d.ts generation
// ---------------------------------------------------------------------------

/// Transitively collects all struct type paths referenced by local modules' public API.
fn collect_referenced_types(
    local_modules: &[RustModule],
    registry: &TypeRegistry,
) -> BTreeSet<String> {
    let mut referenced = BTreeSet::new();
    let mut queue: Vec<String> = Vec::new();

    // Seed: collect all type refs from local module members
    for module in local_modules {
        collect_type_refs_from_module(module, registry, &mut queue);
    }

    // Transitively expand: for each referenced struct, also collect its field type refs
    while let Some(full_path) = queue.pop() {
        if !referenced.insert(full_path.clone()) {
            continue; // already visited
        }
        if let Some(s) = registry.get(&full_path) {
            for field in &s.fields {
                let ts = rust_type_to_ts(&field.type_name, registry);
                let mut field_refs = Vec::new();
                collect_type_refs(&ts, &mut field_refs);
                for (_, fp) in field_refs {
                    if !referenced.contains(&fp) {
                        queue.push(fp);
                    }
                }
            }
        }
    }

    referenced
}

fn collect_type_refs_from_module(
    module: &RustModule,
    registry: &TypeRegistry,
    queue: &mut Vec<String>,
) {
    let members = match &module.members {
        Some(m) => m,
        None => return,
    };
    for member in members {
        match member {
            RustMember::Fn(func) => {
                for arg in &func.args {
                    let ts = rust_type_to_ts(&arg.type_name, registry);
                    let mut refs = Vec::new();
                    collect_type_refs(&ts, &mut refs);
                    queue.extend(refs.into_iter().map(|(_, fp)| fp));
                }
                if !func.return_type_name.is_empty() {
                    let ts = rust_type_to_ts(&func.return_type_name, registry);
                    let mut refs = Vec::new();
                    collect_type_refs(&ts, &mut refs);
                    queue.extend(refs.into_iter().map(|(_, fp)| fp));
                }
            }
            RustMember::Struct(s) => {
                for field in &s.fields {
                    let ts = rust_type_to_ts(&field.type_name, registry);
                    let mut refs = Vec::new();
                    collect_type_refs(&ts, &mut refs);
                    queue.extend(refs.into_iter().map(|(_, fp)| fp));
                }
            }
            RustMember::Module(sub) => {
                collect_type_refs_from_module(sub, registry, queue);
            }
        }
    }
}

fn generate_ambient_dts(
    local_modules: &[RustModule],
    registry: &TypeRegistry,
    crate_name: &str,
) -> String {
    let referenced = collect_referenced_types(local_modules, registry);

    // Group referenced types by their virtual module path
    // crate::kitty::Wrap -> $rust/crate_name/kitty
    // serde_json::value::Value -> $rust/serde_json/value
    let mut by_module: BTreeMap<String, BTreeMap<String, &RustStruct>> = BTreeMap::new();

    for full_path in &referenced {
        let s = match registry.get(full_path.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let short_name = s.name.rsplit("::").next().unwrap_or(&s.name).to_string();

        let virtual_module = if full_path.starts_with("crate::") {
            let without_crate = full_path.strip_prefix("crate::").unwrap();
            let module_path = without_crate
                .rsplit_once("::")
                .map(|(prefix, _)| prefix)
                .unwrap_or(without_crate);
            format!("$rust/{}/{}", crate_name, module_path.replace("::", "/"))
        } else {
            let module_path = full_path
                .rsplit_once("::")
                .map(|(prefix, _)| prefix)
                .unwrap_or(full_path);
            format!("$rust/{}", module_path.replace("::", "/"))
        };

        by_module
            .entry(virtual_module)
            .or_default()
            .entry(short_name)
            .or_insert(s);
    }

    let mut out = String::from("// Auto-generated by rust-curse.\n");

    for (module_name, types) in &by_module {
        writeln!(out).unwrap();
        writeln!(out, "declare module '$rust/{}' {{", module_name).unwrap();
        for (short_name, s) in types {
            let ts_body = struct_to_ts_type(s, registry);
            writeln!(out, "  export type {} = {};", short_name, ts_body).unwrap();
        }
        writeln!(out, "}}").unwrap();
    }

    out
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generates `.d.ts` files for each discovered module under `output_dir`,
/// replicating the relative directory structure from `project_root`.
///
/// Also generates `ambient.d.ts` with virtual module declarations.
pub fn generate_types(
    modules: &[RustModule],
    dep_modules: &HashMap<String, RustModule>,
    output_dir: &Path,
    project_root: &Path,
    crate_name: &str,
) {
    let root = fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let registry = build_type_registry(modules, dep_modules);

    for module in modules {
        if module.members.is_none() {
            continue;
        }

        let module_path = Path::new(&module.path);
        let relative = module_path.strip_prefix(&root).unwrap_or(module_path);

        let dts_relative = if relative.file_name().is_some_and(|n| n == "mod.rs") {
            relative.parent().unwrap_or(relative).join("index.d.ts")
        } else {
            let mut name = relative.as_os_str().to_owned();
            name.push(".d.ts");
            Path::new(&name).to_path_buf()
        };

        let dts_path = output_dir.join(&dts_relative);
        if let Some(parent) = dts_path.parent() {
            fs::create_dir_all(parent).expect("failed to create directory for .d.ts");
        }

        let content = generate_dts_content(module, &registry, crate_name);
        fs::write(&dts_path, &content).expect("failed to write .d.ts file");
    }

    // Generate ambient.d.ts
    let ambient = generate_ambient_dts(modules, &registry, crate_name);
    let ambient_path = output_dir.join("ambient.d.ts");
    fs::write(&ambient_path, &ambient).expect("failed to write ambient.d.ts");
}
