//! IR -> TS printer.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use super::mangle::is_canonical_origin;
use super::asg::{Ctor, Id, Item, ItemPath, Primitive, StructShape, Type, ASG};

const PREAMBLE: &str = "";

/// Options for printing a single module's `.d.ts` body.
pub struct PrintTsModOpts<'a> {
    /// IR id of the module to print.
    pub mod_id: Id,
    /// Source-tree prefix shared by all module files in the build, e.g.
    /// `PathBuf::from("src")`. Module files live at
    /// `<rel_src_path>/<segs-after-crate>.d.ts`.
    pub rel_src_path: &'a Path,
}

impl ASG {
    /// Prints the ambient declarations shared across the build (the
    /// `$rust-curse` module with the `Copy<T>` utility). Per-module bodies
    /// live in their own files; see [`IR::print_ts_mod`].
    pub fn print_ts_ambient(&self) -> &'static str {
        PREAMBLE
    }

    /// Prints the body of a single module's `.d.ts` file (no `declare
    /// module` wrapper). Cross-module imports are emitted as relative
    /// specifiers based on `opts.rel_src_path`.
    pub fn print_ts_mod(&self, opts: PrintTsModOpts<'_>) -> String {
        Printer::new(self, opts.mod_id, opts.rel_src_path).run()
    }
}

struct Printer<'a> {
    ir: &'a ASG,
    mod_id: Id,
    rel_src_path: &'a Path,
    local_structs: HashSet<Id>,
    /// Map of relative import specifier (e.g. `"./types"`) to the set of
    /// type names imported from there.
    imports: BTreeMap<String, BTreeSet<String>>,
}

impl<'a> Printer<'a> {
    fn new(ir: &'a ASG, mod_id: Id, rel_src_path: &'a Path) -> Self {
        Self {
            ir,
            mod_id,
            rel_src_path,
            local_structs: HashSet::new(),
            imports: BTreeMap::new(),
        }
    }

    fn run(&mut self) -> String {
        let mod_segments = self.ir.get_mod_unchecked(self.mod_id).path.segments.clone();
        let exports = self.ir.get_mod_unchecked(self.mod_id).exports.clone();

        for export in &exports {
            if let Item::Struct(def) = self.ir.get_unchecked(export.target) {
                if is_canonical_origin(&def.path, &mod_segments) {
                    self.local_structs.insert(export.target);
                }
            }
        }

        let body = self.print_exports(&mod_segments, &exports);

        let mut out = String::new();
        for (specifier, names) in &self.imports {
            let names_str: Vec<&str> = names.iter().map(String::as_str).collect();
            out.push_str(&format!(
                "import type {{ {} }} from \"{specifier}\";\n",
                names_str.join(", ")
            ));
        }
        if !self.imports.is_empty() {
            out.push('\n');
        }
        out.push_str(&body);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
    }

    fn print_exports(&mut self, mod_segments: &[String], exports: &[super::asg::Export]) -> String {
        let mut parts: Vec<String> = Vec::new();
        for export in exports {
            match self.ir.get_unchecked(export.target) {
                Item::Struct(def) => {
                    let canonical = def.path.clone();
                    if is_canonical_origin(&canonical, mod_segments) {
                        parts.push(self.print_struct(export.target));
                    } else {
                        parts.push(self.print_reexport(&canonical, &export.name));
                    }
                }
                Item::Fn(def) => {
                    let canonical = def.path.clone();
                    if is_canonical_origin(&canonical, mod_segments) {
                        parts.push(self.print_fn(export.target));
                    } else {
                        parts.push(self.print_reexport(&canonical, &export.name));
                    }
                }
                Item::Mod(_) => {}
            }
        }
        parts.join("\n\n")
    }

    fn type_to_ts(&mut self, ty: &Type) -> String {
        match ty {
            Type::Primitive(Primitive::Unit) => "void".into(),
            Type::Primitive(Primitive::Boolean) => "boolean".into(),
            Type::Primitive(Primitive::Char | Primitive::String | Primitive::Str) => {
                "string".into()
            }
            Type::Primitive(
                Primitive::i64 | Primitive::u64 | Primitive::isize | Primitive::usize,
            ) => "bigint".into(),
            Type::Primitive(
                Primitive::i8
                | Primitive::u8
                | Primitive::i16
                | Primitive::u16
                | Primitive::i32
                | Primitive::u32
                | Primitive::f32
                | Primitive::f64,
            ) => "number".into(),
            Type::Struct(sid) => {
                let path = self.ir.get_struct_unchecked(*sid).path.clone();
                let name = path.name().unwrap_or("unknown").to_string();
                if !self.local_structs.contains(sid) {
                    self.record_import(&path);
                }
                name
            }
        }
    }

    fn print_struct(&mut self, sid: Id) -> String {
        let def = self.ir.get_struct_unchecked(sid).clone();
        let name = def.path.name().unwrap_or("unknown").to_string();
        let brand = def.path.segments.join("::");

        let mut members: Vec<String> = Vec::new();

        members.push(format!("    private [\" \"]: '{brand}';"));

        if let StructShape::Struct(fields) = &def.shape {
            for field in fields {
                let ty = self.type_to_ts(&field.ty);
                let mut member = String::new();
                if let Some(docs) = &field.docs {
                    member.push_str(&doc_comment(docs, "    "));
                }
                member.push_str(&format!("    {}: {ty};", field.name));
                members.push(member);
            }
        } else if let StructShape::Tuple(types) = &def.shape {
            for (i, ty) in types.iter().enumerate() {
                let ty = self.type_to_ts(ty);
                members.push(format!("    {i}: {ty};"));
            }
        }

        let ctor = match &def.ctor {
            Ctor::Explicit(fid) => {
                let fid = *fid;
                let params: Vec<String> = self
                    .ir
                    .get_fn_unchecked(fid)
                    .params
                    .clone()
                    .iter()
                    .map(|p| {
                        let ty = self.type_to_ts(&p.ty);
                        format!("{}: {ty}", p.name)
                    })
                    .collect();
                format!("    constructor({});", params.join(", "))
            }
            Ctor::Implicit => {
                let params = self.shape_ctor_params(&def.shape);
                format!("    constructor({});", params.join(", "))
            }
        };
        members.push(ctor);

        let mut out = String::new();
        if let Some(docs) = &def.docs {
            out.push_str(&doc_comment(docs, ""));
        }
        out.push_str(&format!("export class {name} {{\n"));
        out.push_str(&members.join("\n\n"));
        out.push('\n');
        out.push('}');
        out
    }

    fn shape_ctor_params(&mut self, shape: &StructShape) -> Vec<String> {
        match shape {
            StructShape::Unit => vec![],
            StructShape::Tuple(types) => types
                .iter()
                .enumerate()
                .map(|(i, ty)| {
                    let ty = self.type_to_ts(ty);
                    format!("_{i}: {ty}")
                })
                .collect(),
            StructShape::Struct(fields) => fields
                .iter()
                .map(|f| {
                    let ty = self.type_to_ts(&f.ty);
                    format!("{}: {ty}", f.name)
                })
                .collect(),
        }
    }

    fn print_fn(&mut self, fid: Id) -> String {
        let def = self.ir.get_fn_unchecked(fid).clone();
        let name = def.path.name().unwrap_or("unknown").to_string();
        let mut out = String::new();

        if let Some(docs) = &def.docs {
            out.push_str(&doc_comment(docs, ""));
        }

        let params: Vec<String> = def
            .params
            .iter()
            .map(|p| {
                let ty = self.type_to_ts(&p.ty);
                format!("{}: {ty}", p.name)
            })
            .collect();

        let ret = self.type_to_ts(&def.return_type);
        out.push_str(&format!(
            "export function {name}({}): {ret};",
            params.join(", ")
        ));
        out
    }

    fn print_reexport(&mut self, canonical: &ItemPath, local_name: &str) -> String {
        let canonical_name = canonical.name().unwrap_or(local_name).to_string();
        let target_mod_segments = &canonical.segments[..canonical.segments.len() - 1];
        let specifier = self.specifier_to_module(target_mod_segments);
        if canonical_name == local_name {
            format!("export {{ {local_name} }} from \"{specifier}\";")
        } else {
            format!("export {{ {canonical_name} as {local_name} }} from \"{specifier}\";")
        }
    }

    fn record_import(&mut self, path: &ItemPath) {
        if path.segments.len() < 2 {
            return;
        }
        let type_name = path.name().unwrap().to_string();
        let target_mod_segments = &path.segments[..path.segments.len() - 1];
        let specifier = self.specifier_to_module(target_mod_segments);
        self.imports.entry(specifier).or_default().insert(type_name);
    }

    /// Resolve a relative import specifier from this printer's current
    /// module to the module identified by `target_mod_segments`.
    fn specifier_to_module(&self, target_mod_segments: &[String]) -> String {
        let from = mod_file_path(
            self.rel_src_path,
            &self.ir.get_mod_unchecked(self.mod_id).path.segments,
        );
        let to = mod_file_path(self.rel_src_path, target_mod_segments);
        relative_specifier(&from, &to)
    }
}

/// Compute the `.rs.d.ts` file path for a module given its full segment
/// path (the first segment is the crate name and is dropped). The crate
/// root (`segments.len() == 1`) maps to `<rel_src_path>/lib.rs.d.ts`,
/// mirroring `src/lib.rs`. Otherwise the remaining segments become
/// directory components, with the last as the file stem.
fn mod_file_path(rel_src_path: &Path, segments: &[String]) -> PathBuf {
    assert!(
        !segments.is_empty(),
        "mod_file_path requires non-empty segments"
    );
    let mut path = rel_src_path.to_path_buf();
    if segments.len() == 1 {
        path.push("lib.rs.d.ts");
        return path;
    }
    for seg in &segments[1..segments.len() - 1] {
        path.push(seg);
    }
    path.push(format!("{}.rs.d.ts", segments.last().unwrap()));
    path
}

/// Compute a TS-style relative import specifier from `from_file` to
/// `to_file`. Both are paths to `.rs.d.ts` files. The result keeps the
/// `.rs` extension (so TS resolves it via `allowArbitraryExtensions`) and
/// is prefixed with `./` for sibling/descendant targets.
fn relative_specifier(from_file: &Path, to_file: &Path) -> String {
    let from_dir: Vec<&str> = from_file
        .parent()
        .map(|p| p.iter().filter_map(|s| s.to_str()).collect())
        .unwrap_or_default();
    let to_components: Vec<&str> = to_file.iter().filter_map(|s| s.to_str()).collect();

    let common_prefix_len = from_dir
        .iter()
        .zip(to_components.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let ups = from_dir.len() - common_prefix_len;
    let mut downs: Vec<&str> = to_components[common_prefix_len..].to_vec();

    let last = downs.pop().expect("to_file has at least one component");
    let stem = last
        .strip_suffix(".rs.d.ts")
        .expect("module file ends with .rs.d.ts");

    let mut spec = String::new();
    if ups == 0 {
        spec.push_str("./");
    } else {
        for _ in 0..ups {
            spec.push_str("../");
        }
    }
    for d in &downs {
        spec.push_str(d);
        spec.push('/');
    }
    spec.push_str(stem);
    spec.push_str(".rs");
    spec
}

fn doc_comment(docs: &str, indent: &str) -> String {
    let trimmed = docs.trim();
    if trimmed.contains('\n') {
        let lines: Vec<String> = trimmed
            .lines()
            .map(|l| format!("{indent} * {}", l.trim()))
            .collect();
        format!("{indent}/**\n{}\n{indent} */\n", lines.join("\n"))
    } else {
        format!("{indent}/** {trimmed} */\n")
    }
}
