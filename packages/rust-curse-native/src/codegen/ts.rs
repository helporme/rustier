//! IR -> TS printer.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use super::ir::{
    ClassShape, EnumVariantDef, FnDef, FnParam, Ir, IrId, Item, ItemPath, ParamMode, Primitive,
    Receiver, Span, Type, Use,
};
use super::mangle::is_canonical_origin;
use super::source_map::Mapping;
use crate::common::coalesce_enum_fields;

const PREAMBLE: &str = "";

/// Options for printing a single module's `.d.ts` body.
pub struct PrintTsModOpts<'a> {
    /// IR id of the module to print.
    pub mod_id: IrId,
    /// Source-tree prefix shared by all module files in the build, e.g.
    /// `PathBuf::from("src")`. Module files live at
    /// `<rel_src_path>/<segs-after-crate>.d.ts`.
    pub rel_src_path: &'a Path,
}

/// A printed `.d.ts` body together with the source-map data.
pub struct PrintedTsMod {
    pub body: String,
    pub mappings: Vec<Mapping>,
    pub source_rel_path: Option<PathBuf>,
}

impl Ir {
    /// Prints the ambient declarations shared across the build (the
    /// `$rust-curse` module with the `Copy<T>` utility). Per-module bodies
    /// live in their own files; see [`Ir::print_ts_mod`].
    pub fn print_ts_ambient(&self) -> &'static str {
        PREAMBLE
    }

    /// Prints the body of a single module's `.d.ts` file (no `declare
    /// module` wrapper). Cross-module imports are emitted as relative
    /// specifiers based on `opts.rel_src_path`. Returns the body plus
    /// source-map mappings anchored at user-visible JS identifiers.
    pub fn print_ts_mod(&self, opts: PrintTsModOpts<'_>) -> PrintedTsMod {
        Printer::new(self, opts.mod_id, opts.rel_src_path).run()
    }
}

struct Writer {
    buf: String,
    line: u32,
    col: u32,
}

impl Writer {
    fn new() -> Self {
        Self {
            buf: String::new(),
            line: 0,
            col: 0,
        }
    }

    fn push_str(&mut self, s: &str) {
        for ch in s.chars() {
            if ch == '\n' {
                self.line += 1;
                self.col = 0;
            } else {
                self.col += 1;
            }
        }
        self.buf.push_str(s);
    }

    fn push(&mut self, ch: char) {
        if ch == '\n' {
            self.line += 1;
            self.col = 0;
        } else {
            self.col += 1;
        }
        self.buf.push(ch);
    }

    fn here(&self) -> (u32, u32) {
        (self.line, self.col)
    }
}

struct Printer<'a> {
    ir: &'a Ir,
    mod_id: IrId,
    rel_src_path: &'a Path,
    local_classes: HashSet<IrId>,
    /// Map of relative import specifier (e.g. `"./types"`) to the set of
    /// type names imported from there.
    imports: BTreeMap<String, BTreeSet<String>>,
    mappings: Vec<Mapping>,
    body: Writer,
}

impl<'a> Printer<'a> {
    fn new(ir: &'a Ir, mod_id: IrId, rel_src_path: &'a Path) -> Self {
        Self {
            ir,
            mod_id,
            rel_src_path,
            local_classes: HashSet::new(),
            imports: BTreeMap::new(),
            mappings: Vec::new(),
            body: Writer::new(),
        }
    }

    fn run(mut self) -> PrintedTsMod {
        let mod_def = self.ir.get_mod_unchecked(self.mod_id).clone();
        let mod_segments = mod_def.path.segments.clone();
        let uses = mod_def.uses.clone();

        for use_ in &uses {
            if let Item::Class(def) = self.ir.get_unchecked(use_.target) {
                if is_canonical_origin(&def.path, &mod_segments) {
                    self.local_classes.insert(use_.target);
                }
            }
        }

        self.print_uses(&mod_segments, &uses);

        let mut prologue = String::new();
        for (specifier, names) in &self.imports {
            let names_str: Vec<&str> = names.iter().map(String::as_str).collect();
            prologue.push_str(&format!(
                "import type {{ {} }} from \"{specifier}\";\n",
                names_str.join(", ")
            ));
        }
        if !self.imports.is_empty() {
            prologue.push('\n');
        }
        let prologue_lines = prologue.matches('\n').count() as u32;
        for m in &mut self.mappings {
            m.gen_line += prologue_lines;
        }

        let mut out = prologue;
        out.push_str(&self.body.buf);
        if !out.ends_with('\n') {
            out.push('\n');
        }

        PrintedTsMod {
            body: out,
            mappings: self.mappings,
            source_rel_path: mod_def.span.as_ref().map(|s| s.filename.clone()),
        }
    }

    fn print_uses(&mut self, mod_segments: &[String], uses: &[Use]) {
        let mut first = true;
        for use_ in uses {
            match self.ir.get_unchecked(use_.target) {
                Item::Class(def) => {
                    let canonical = def.path.clone();
                    if is_canonical_origin(&canonical, mod_segments) {
                        self.gap(&mut first);
                        self.print_class(use_.target);
                    } else {
                        self.gap(&mut first);
                        self.print_reexport(&canonical, &use_.name);
                    }
                }
                Item::Fn(def) => {
                    let canonical = def.path.clone();
                    if is_canonical_origin(&canonical, mod_segments) {
                        self.gap(&mut first);
                        self.print_fn(use_.target);
                    } else {
                        self.gap(&mut first);
                        self.print_reexport(&canonical, &use_.name);
                    }
                }
                Item::Mod(_) => {}
            }
        }
    }

    fn gap(&mut self, first: &mut bool) {
        if *first {
            *first = false;
        } else {
            self.body.push_str("\n\n");
        }
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
            Type::Class(cid) => {
                let path = self.ir.get_class_unchecked(*cid).path.clone();
                let name = path.name().unwrap_or("unknown").to_string();
                if !self.local_classes.contains(cid) {
                    self.record_import(&path);
                }
                name
            }
        }
    }

    fn param_type_to_ts(&mut self, p: &FnParam) -> String {
        let base = self.type_to_ts(&p.ty);
        if let Type::Class(cid) = &p.ty {
            if matches!(p.mode, ParamMode::ByMut) {
                return base;
            }
            if self.ir.get_class_unchecked(*cid).struct_expr.is_some() {
                return format!("{base} | {base}.Init");
            }
        }
        base
    }

    fn print_class(&mut self, cid: IrId) {
        if matches!(
            self.ir.get_class_unchecked(cid).shape,
            ClassShape::Enum { .. }
        ) {
            self.print_enum_class(cid);
            return;
        }
        let def = self.ir.get_class_unchecked(cid).clone();
        let name = def.js_name.clone();
        let brand = def.path.segments.join("::");
        let ClassShape::Struct { fields } = &def.shape else {
            unreachable!("enum shape handled by print_enum_class")
        };

        if let Some(docs) = &def.docs {
            self.body.push_str(&doc_comment(docs, ""));
        }
        self.body.push_str("export class ");
        self.anchor(def.span.as_ref(), 0);
        self.body.push_str(&name);
        self.body.push_str(" {\n");

        self.body
            .push_str(&format!("    private [\" \"]: '{brand}';"));

        let mut first_member = false; // brand line already emitted

        for field in fields {
            self.print_field(field, &mut first_member);
        }

        if let Some(ctor) = &def.ctor {
            self.member_gap(&mut first_member);
            self.print_ctor(ctor, &def);
        } else {
            self.member_gap(&mut first_member);
            self.body.push_str("    private constructor();");
        }

        if def.struct_expr.is_some() {
            self.member_gap(&mut first_member);
            self.body.push_str("    static ");
            // The auto-`of` factory has no better source anchor than the
            // struct itself.
            self.anchor(def.span.as_ref(), 0);
            let of_name = if def.of_escaped { "of$" } else { "of" };
            self.body
                .push_str(&format!("{of_name}(init: {name}.Init): {name};"));
        }

        for mid in &def.methods {
            let m = self.ir.get_fn_unchecked(*mid).clone();
            self.member_gap(&mut first_member);
            self.print_method(&m);
        }

        self.body.push('\n');
        self.body.push('}');

        if let Some(se) = def.struct_expr.clone() {
            self.body.push_str("\n\n");
            if let Some(docs) = &def.docs {
                self.body.push_str(&doc_comment(docs, ""));
            }
            self.body.push_str("export namespace ");
            self.anchor(def.span.as_ref(), 0);
            self.body.push_str(&name);
            self.body.push_str(" {\n");
            self.body.push_str("    export type Init = {\n");
            for f in &se.fields {
                let ty = self.field_init_type(&f.ty);
                let class_field = fields.iter().find(|cf| cf.orig_name == f.name);
                let docs = class_field.and_then(|cf| cf.docs.clone());
                if let Some(docs) = &docs {
                    self.body.push_str(&doc_comment(docs, "        "));
                }
                self.body.push_str("        ");
                let span = class_field.and_then(|cf| cf.span.as_ref());
                self.anchor(span, 0);
                self.body.push_str(&format!("{}: {ty};\n", f.name));
            }
            self.body.push_str("    };\n");
            self.body.push('}');
        }
    }

    /// Emit the constructor-signature surface for a data enum:
    /// `export type E = E.Instance & E.Init;`, `export declare const E`,
    /// and `export declare namespace E { interface Instance; type Init }`.
    fn print_enum_class(&mut self, cid: IrId) {
        let def = self.ir.get_class_unchecked(cid).clone();
        let name = def.js_name.clone();
        let brand = def.path.segments.join("::");
        let ClassShape::Enum {
            variants,
            discriminator,
        } = &def.shape
        else {
            unreachable!("print_enum_class on non-enum")
        };

        // export type E = E.Instance & E.Init;
        if let Some(docs) = &def.docs {
            self.body.push_str(&doc_comment(docs, ""));
        }
        self.body.push_str("export type ");
        self.anchor(def.span.as_ref(), 0);
        self.body
            .push_str(&format!("{name} = {name}.Instance & {name}.Init;\n\n"));

        // export declare const E: { ... };
        self.body.push_str("export declare const ");
        self.anchor(def.span.as_ref(), 0);
        self.body.push_str(&name);
        self.body.push_str(": {\n");
        self.body
            .push_str(&format!("    readonly prototype: {name};\n"));
        if let Some(ctor) = &def.ctor {
            let params: Vec<String> = ctor
                .params
                .iter()
                .map(|p| format!("{}: {}", p.name, self.param_type_to_ts(p)))
                .collect();
            self.body.push_str("    ");
            self.anchor(ctor.span.as_ref(), 0);
            self.body
                .push_str(&format!("new ({}): {name};\n", params.join(", ")));
        }
        if def.struct_expr.is_some() {
            let of_name = if def.of_escaped { "of$" } else { "of" };
            self.body.push_str("    ");
            self.anchor(def.span.as_ref(), 0);
            self.body
                .push_str(&format!("{of_name}(init: {name}.Init): {name};\n"));
        }
        for mid in &def.methods {
            let m = self.ir.get_fn_unchecked(*mid).clone();
            if m.receiver.is_some() {
                continue;
            }
            self.print_enum_value_method(&m, &name);
        }
        self.body.push_str("};\n\n");

        // export declare namespace E { interface Instance; type Init }
        self.body.push_str("export declare namespace ");
        self.anchor(def.span.as_ref(), 0);
        self.body.push_str(&name);
        self.body.push_str(" {\n");
        self.body.push_str("    export interface Instance {\n");
        self.body
            .push_str(&format!("        readonly [\" \"]: '{brand}';\n"));
        let num_variants = variants.len();
        for cf in coalesce_enum_fields(variants) {
            let mut getter_ty = self.type_to_ts(&cf.ty);
            if cf.carriers.len() < num_variants {
                getter_ty.push_str(" | undefined");
            }
            self.body
                .push_str(&format!("        get {}(): {getter_ty};\n", cf.js_name));
            if cf.has_setter {
                let setter_ty = self.field_init_type(&cf.ty);
                self.body.push_str(&format!(
                    "        set {}(value: {setter_ty});\n",
                    cf.js_name
                ));
            }
        }
        for mid in &def.methods {
            let m = self.ir.get_fn_unchecked(*mid).clone();
            if m.receiver.is_none() {
                continue;
            }
            let params: Vec<String> = m
                .params
                .iter()
                .map(|p| format!("{}: {}", p.name, self.param_type_to_ts(p)))
                .collect();
            let ret = self.type_to_ts(&m.return_type);
            if let Some(docs) = &m.docs {
                self.body.push_str(&doc_comment(docs, "        "));
            }
            self.body.push_str("        ");
            self.anchor(m.span.as_ref(), 0);
            self.body
                .push_str(&format!("{}({}): {ret};\n", m.js_name, params.join(", ")));
        }
        self.body.push_str("    }\n\n");

        self.body.push_str("    export type Init =\n");
        for (i, v) in variants.iter().enumerate() {
            let last = i + 1 == variants.len();
            self.print_enum_init_arm(v, discriminator, last);
        }
        self.body.push_str("}");
    }

    fn print_enum_value_method(&mut self, m: &FnDef, _name: &str) {
        let params: Vec<String> = m
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, self.param_type_to_ts(p)))
            .collect();
        let ret = self.type_to_ts(&m.return_type);
        if let Some(docs) = &m.docs {
            self.body.push_str(&doc_comment(docs, "    "));
        }
        self.body.push_str("    ");
        self.anchor(m.span.as_ref(), 0);
        self.body
            .push_str(&format!("{}({}): {ret};\n", m.js_name, params.join(", ")));
    }

    fn print_enum_init_arm(&mut self, v: &EnumVariantDef, discriminator: &str, last: bool) {
        self.body.push_str("        | {\n");
        // The variant docs anchor on the discriminator literal of this arm.
        if let Some(docs) = &v.docs {
            self.body.push_str(&doc_comment(docs, "              "));
        }
        self.body.push_str("              ");
        self.anchor(v.span.as_ref(), 0);
        self.body
            .push_str(&format!("readonly {discriminator}: '{}';\n", v.tag));
        for f in &v.fields {
            let ty = self.field_init_type(&f.ty);
            if let Some(docs) = &f.docs {
                self.body.push_str(&doc_comment(docs, "              "));
            }
            self.body.push_str("              ");
            self.anchor(f.span.as_ref(), 0);
            self.body.push_str(&format!("{}: {ty};\n", f.js_name));
        }
        if last {
            self.body.push_str("          };\n");
        } else {
            self.body.push_str("          }\n");
        }
    }

    fn member_gap(&mut self, first: &mut bool) {
        if *first {
            *first = false;
        } else {
            self.body.push_str("\n\n");
        }
    }

    fn print_field(&mut self, field: &super::ir::ClassField, first: &mut bool) {
        let docs = field.docs.clone();
        if field.has_getter {
            self.member_gap(first);
            if let Some(docs) = &docs {
                self.body.push_str(&doc_comment(docs, "    "));
            }
            let ty = self.type_to_ts(&field.ty);
            self.body.push_str("    ");
            self.anchor(field.span.as_ref(), 0);
            self.body
                .push_str(&format!("get {}(): {ty};", field.js_name));
        }
        if field.has_setter {
            self.member_gap(first);
            if !field.has_getter {
                if let Some(docs) = &docs {
                    self.body.push_str(&doc_comment(docs, "    "));
                }
            }
            let ty = self.field_init_type(&field.ty);
            self.body.push_str("    ");
            self.anchor(field.span.as_ref(), 0);
            self.body
                .push_str(&format!("set {}(value: {ty});", field.js_name));
        }
    }

    fn print_ctor(&mut self, ctor: &super::ir::CtorDef, _def: &super::ir::ClassDef) {
        let ctor_params: Vec<String> = ctor
            .params
            .iter()
            .map(|p| {
                let ty = self.param_type_to_ts(p);
                format!("{}: {ty}", p.name)
            })
            .collect();
        self.body.push_str("    ");
        self.anchor(ctor.span.as_ref(), 0);
        self.body
            .push_str(&format!("constructor({});", ctor_params.join(", ")));
    }

    fn print_method(&mut self, def: &FnDef) {
        let name = &def.js_name;
        let params: Vec<String> = def
            .params
            .iter()
            .map(|p| {
                let ty = self.param_type_to_ts(p);
                format!("{}: {ty}", p.name)
            })
            .collect();
        let ret = self.type_to_ts(&def.return_type);
        let prefix = match def.receiver {
            None => "static ",
            Some(Receiver::ByRef | Receiver::ByMut | Receiver::ByValue) => "",
        };
        if let Some(docs) = &def.docs {
            self.body.push_str(&doc_comment(docs, "    "));
        }
        self.body.push_str("    ");
        self.body.push_str(prefix);
        self.anchor(def.span.as_ref(), 0);
        self.body
            .push_str(&format!("{name}({}): {ret};", params.join(", ")));
    }

    /// Init-position field type: struct fields become `X | X.Init` (the same
    /// relaxed shape we accept on params).
    fn field_init_type(&mut self, ty: &Type) -> String {
        let base = self.type_to_ts(ty);
        if let Type::Class(cid) = ty {
            if self.ir.get_class_unchecked(*cid).struct_expr.is_some() {
                return format!("{base} | {base}.Init");
            }
        }
        base
    }

    fn print_fn(&mut self, fid: IrId) {
        let def = self.ir.get_fn_unchecked(fid).clone();
        let name = &def.js_name;

        if let Some(docs) = &def.docs {
            self.body.push_str(&doc_comment(docs, ""));
        }

        let params: Vec<String> = def
            .params
            .iter()
            .map(|p| {
                let ty = self.param_type_to_ts(p);
                format!("{}: {ty}", p.name)
            })
            .collect();

        let ret = self.type_to_ts(&def.return_type);
        self.body.push_str("export function ");
        self.anchor(def.span.as_ref(), 0);
        self.body
            .push_str(&format!("{name}({}): {ret};", params.join(", ")));
    }

    fn print_reexport(&mut self, canonical: &ItemPath, local_name: &str) {
        let canonical_name = canonical.name().unwrap_or(local_name).to_string();
        let target_mod_segments = &canonical.segments[..canonical.segments.len() - 1];
        let specifier = self.specifier_to_module(target_mod_segments);
        if canonical_name == local_name {
            self.body
                .push_str(&format!("export {{ {local_name} }} from \"{specifier}\";"));
        } else {
            self.body.push_str(&format!(
                "export {{ {canonical_name} as {local_name} }} from \"{specifier}\";"
            ));
        }
    }

    /// Record a mapping at the current writer position pointing to `span`.
    /// `src_idx` is fixed at 0 today -> a module always has exactly one
    /// source. `span.begin` is 1-indexed; we convert to 0-indexed here.
    /// The anchor lands at the span's start (which rustdoc places at the
    /// item keyword, e.g. `pub`). Refining to the identifier itself is a
    /// follow-up -> would need source-file access, kept out of the printer.
    fn anchor(&mut self, span: Option<&Span>, src_idx: u32) {
        let Some(span) = span else { return };
        let (gen_line, gen_col) = self.body.here();
        let (src_line_1, src_col_1) = span.begin;
        self.mappings.push(Mapping {
            gen_line,
            gen_col,
            src_idx,
            src_line: src_line_1.saturating_sub(1) as u32,
            src_col: src_col_1.saturating_sub(1) as u32,
        });
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
