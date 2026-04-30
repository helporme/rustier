//! IR -> TS printer.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use super::mangle::is_canonical_origin;
use super::{Ctor, FnParam, Id, Item, ItemPath, NumberType, ParamMode, StructShape, Type, IR};

const PREAMBLE: &str = r#"declare module "$rust-curse" {
    export type Copy<T> = {
        [K in keyof T as T[K] extends
            | string | number | boolean | bigint
            | Record<string, unknown> | unknown[]
            ? K : never]: Copy<T[K]>;
    };
}"#;

impl IR {
    /// Prints the IR as TypeScript ambient module code, featuring _all_
    /// exported items.
    pub fn print_ts(&self) -> String {
        Printer::new(self).run()
    }
}

struct Printer<'a> {
    ir: &'a IR,
    local_structs: HashSet<Id>,
    imports: BTreeMap<String, BTreeSet<String>>,
    needs_copy: bool,
}

impl<'a> Printer<'a> {
    fn new(ir: &'a IR) -> Self {
        Self {
            ir,
            local_structs: HashSet::new(),
            imports: BTreeMap::new(),
            needs_copy: false,
        }
    }

    fn run(&mut self) -> String {
        let mut out = String::from(PREAMBLE);
        let mod_ids: Vec<Id> = self
            .ir
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, it)| matches!(it, Item::Mod(_)).then_some(Id(i as u32)))
            .collect();
        for mid in mod_ids {
            let block = self.print_mod_block(mid);
            if !block.is_empty() {
                out.push('\n');
                out.push('\n');
                out.push_str(&block);
            }
        }
        out
    }

    fn print_mod_block(&mut self, mid: Id) -> String {
        self.imports.clear();
        self.needs_copy = false;

        self.local_structs.clear();
        let mod_segments = self.ir.get_mod_unchecked(mid).path.segments.clone();
        let exports = self.ir.get_mod_unchecked(mid).exports.clone();
        for export in &exports {
            if let Item::Struct(def) = self.ir.get_unchecked(export.target) {
                if is_canonical_origin(&def.path, &mod_segments) {
                    self.local_structs.insert(export.target);
                }
            }
        }

        let body = self.print_mod_exports(mid);
        if body.is_empty() {
            return String::new();
        }

        let module_path = mod_ambient_path(self.ir.get_mod_unchecked(mid));

        let mut block = format!("declare module \"{module_path}\" {{\n");

        if self.needs_copy {
            block.push_str("    import type { Copy } from \"$rust-curse\";\n");
        }
        for (path, names) in &self.imports {
            let names_str: Vec<&str> = names.iter().map(String::as_str).collect();
            block.push_str(&format!(
                "    import type {{ {} }} from \"{path}\";\n",
                names_str.join(", ")
            ));
        }
        if self.needs_copy || !self.imports.is_empty() {
            block.push('\n');
        }

        block.push_str(&body);
        block.push('\n');
        block.push('}');
        block
    }

    fn print_mod_exports(&mut self, mid: Id) -> String {
        let mod_segments = self.ir.get_mod_unchecked(mid).path.segments.clone();
        let exports = self.ir.get_mod_unchecked(mid).exports.clone();
        let mut parts: Vec<String> = Vec::new();
        for export in &exports {
            match self.ir.get_unchecked(export.target) {
                Item::Struct(def) => {
                    let canonical = def.path.clone();
                    if is_canonical_origin(&canonical, &mod_segments) {
                        parts.push(self.print_struct(export.target));
                    } else {
                        parts.push(print_reexport(&canonical, &export.name));
                    }
                }
                Item::Fn(def) => {
                    let canonical = def.path.clone();
                    if is_canonical_origin(&canonical, &mod_segments) {
                        parts.push(self.print_fn(export.target));
                    } else {
                        parts.push(print_reexport(&canonical, &export.name));
                    }
                }
                Item::Mod(_) => {}
            }
        }
        parts.join("\n\n")
    }

    fn type_to_ts(&mut self, ty: &Type) -> String {
        match ty {
            Type::Unit => "void".into(),
            Type::Boolean => "boolean".into(),
            Type::Char | Type::String | Type::Str => "string".into(),
            Type::Number(n) => match n {
                NumberType::i64 | NumberType::u64 | NumberType::isize | NumberType::usize => {
                    "bigint".into()
                }
                _ => "number".into(),
            },
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

    fn param_to_ts(&mut self, param: &FnParam) -> String {
        let base = self.type_to_ts(&param.ty);
        let is_nominal = matches!(param.ty, Type::Struct(_));
        if param.mode == ParamMode::ByValue && is_nominal {
            self.needs_copy = true;
            format!("Copy<{base}>")
        } else {
            base
        }
    }

    fn print_struct(&mut self, sid: Id) -> String {
        let def = self.ir.get_struct_unchecked(sid).clone();
        let name = def.path.name().unwrap_or("unknown").to_string();
        let brand = def.path.segments.join("::");

        let mut members: Vec<String> = Vec::new();

        members.push(format!("        private [\" \"]: '{brand}';"));

        if let StructShape::Struct(fields) = &def.shape {
            for field in fields {
                let ty = self.type_to_ts(&field.ty);
                let mut member = String::new();
                if let Some(docs) = &field.docs {
                    member.push_str(&doc_comment(docs, "        "));
                }
                member.push_str(&format!("        {}: {ty};", field.name));
                members.push(member);
            }
        } else if let StructShape::Tuple(types) = &def.shape {
            for (i, ty) in types.iter().enumerate() {
                let ty = self.type_to_ts(ty);
                members.push(format!("        {i}: {ty};"));
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
                format!("        constructor({});", params.join(", "))
            }
            Ctor::Implicit => {
                let params = self.shape_ctor_params(&def.shape);
                format!("        constructor({});", params.join(", "))
            }
        };
        members.push(ctor);

        let mut out = String::new();
        if let Some(docs) = &def.docs {
            out.push_str(&doc_comment(docs, "    "));
        }
        out.push_str(&format!("    export class {name} {{\n"));
        out.push_str(&members.join("\n\n"));
        out.push('\n');
        out.push_str("    }");
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
            out.push_str(&doc_comment(docs, "    "));
        }

        let params: Vec<String> = def
            .params
            .iter()
            .map(|p| {
                let ty = self.param_to_ts(p);
                format!("{}: {ty}", p.name)
            })
            .collect();

        let ret = self.type_to_ts(&def.return_type);
        out.push_str(&format!(
            "    export function {name}({}): {ret};",
            params.join(", ")
        ));
        out
    }

    fn record_import(&mut self, path: &ItemPath) {
        if path.segments.len() < 2 {
            return;
        }
        let type_name = path.name().unwrap().to_string();
        let module_segments = &path.segments[..path.segments.len() - 1];
        let module_path = format!("$rust/{}", module_segments.join("/"));
        self.imports
            .entry(module_path)
            .or_default()
            .insert(type_name);
    }
}

fn mod_ambient_path(mod_: &super::ModDef) -> String {
    format!("$rust/{}", mod_.path.segments.join("/"))
}

fn print_reexport(canonical: &ItemPath, local_name: &str) -> String {
    let canonical_name = canonical.name().unwrap_or(local_name);
    let module_segments = &canonical.segments[..canonical.segments.len() - 1];
    let source = format!("$rust/{}", module_segments.join("/"));
    if canonical_name == local_name {
        format!("    export {{ {local_name} }} from \"{source}\";")
    } else {
        format!("    export {{ {canonical_name} as {local_name} }} from \"{source}\";")
    }
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
