use std::collections::{BTreeMap, BTreeSet, HashSet};

use super::{
    Ctor, ExportKind, FnId, FnParam, ItemPath, ModId, NumberType, ParamMode, StructBoundary,
    StructId, StructShape, Type, TypeId, IR,
};

const PREAMBLE: &str = r#"declare module "$rust-curse" {
    export type Copy<T> = {
        [K in keyof T as T[K] extends
            | string | number | boolean | bigint
            | Record<string, unknown> | unknown[]
            ? K : never]: Copy<T[K]>;
    };
}"#;

impl IR {
    pub fn print_ts(&self) -> String {
        Printer::new(self).run()
    }
}

struct Printer<'a> {
    ir: &'a IR,
    local_structs: HashSet<StructId>,
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
        for i in 0..self.ir.mods.len() {
            let mid = ModId(i as u32);
            let block = self.print_mod_block(mid);
            if !block.is_empty() {
                out.push('\n');
                out.push('\n');
                out.push_str(&block);
            }
        }
        out
    }

    fn print_mod_block(&mut self, mid: ModId) -> String {
        self.imports.clear();
        self.needs_copy = false;

        self.local_structs.clear();
        let mod_segments = self.ir.mod_(mid).path.segments.clone();
        let exports = self.ir.mod_(mid).exports.clone();
        for export in &exports {
            match &export.kind {
                ExportKind::Struct(sid) => {
                    if is_canonical_origin(&self.ir.struct_(*sid).path, &mod_segments) {
                        self.local_structs.insert(*sid);
                    }
                }
                _ => {}
            }
        }

        let body = self.print_mod_exports(mid);
        if body.is_empty() {
            return String::new();
        }

        let module_path = mod_ambient_path(self.ir.mod_(mid));

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

    fn print_mod_exports(&mut self, mid: ModId) -> String {
        let mod_segments = self.ir.mod_(mid).path.segments.clone();
        let exports = self.ir.mod_(mid).exports.clone();
        let mut parts: Vec<String> = Vec::new();
        for export in &exports {
            match &export.kind {
                ExportKind::Struct(sid) => {
                    let canonical = self.ir.struct_(*sid).path.clone();
                    if is_canonical_origin(&canonical, &mod_segments) {
                        parts.push(self.print_struct(*sid));
                    } else {
                        parts.push(print_reexport(&canonical, &export.name));
                    }
                }
                ExportKind::Fn(fid) => {
                    let canonical = self.ir.fn_(*fid).path.clone();
                    if is_canonical_origin(&canonical, &mod_segments) {
                        parts.push(self.print_fn(*fid));
                    } else {
                        parts.push(print_reexport(&canonical, &export.name));
                    }
                }
                ExportKind::Mod(_) => {}
            }
        }
        parts.join("\n\n")
    }

    fn type_to_ts(&mut self, id: TypeId) -> String {
        match self.ir.type_(id).clone() {
            Type::Unit => "void".into(),
            Type::Boolean => "boolean".into(),
            Type::Char | Type::String | Type::Str => "string".into(),
            Type::Number(n) => match n {
                NumberType::i64 | NumberType::u64 | NumberType::isize | NumberType::usize => {
                    "bigint".into()
                }
                _ => "number".into(),
            },
            Type::Option(inner) => {
                let inner_ts = self.type_to_ts(inner);
                format!("{inner_ts} | undefined")
            }
            Type::Result(ok, _) => self.type_to_ts(ok),
            Type::Vec(inner) | Type::Slice(inner) | Type::Array(inner, _) => {
                let inner_ts = self.type_to_ts(inner);
                format!("{}[]", parens_if_union(inner_ts))
            }
            Type::HashSet(inner) | Type::BTreeSet(inner) => {
                let inner_ts = self.type_to_ts(inner);
                format!("Set<{inner_ts}>")
            }
            Type::Tuple(items) => {
                let parts: Vec<String> = items.iter().map(|&id| self.type_to_ts(id)).collect();
                format!("[{}]", parts.join(", "))
            }
            Type::HashMap(k, v) | Type::BTreeMap(k, v) => {
                let k_ts = self.type_to_ts(k);
                let v_ts = self.type_to_ts(v);
                format!("Map<{k_ts}, {v_ts}>")
            }
            Type::Struct(sid) => {
                let path = self.ir.struct_(sid).path.clone();
                let name = path.name().unwrap_or("unknown").to_string();
                if !self.local_structs.contains(&sid) {
                    self.record_import(&path);
                }
                name
            }
        }
    }

    fn param_to_ts(&mut self, param: &FnParam) -> String {
        let base = self.type_to_ts(param.type_id);
        let is_nominal = matches!(self.ir.type_(param.type_id), Type::Struct(_));
        if param.mode == ParamMode::ByValue && is_nominal {
            self.needs_copy = true;
            format!("Copy<{base}>")
        } else {
            base
        }
    }

    fn print_struct(&mut self, sid: StructId) -> String {
        let def = self.ir.struct_(sid).clone();
        let name = def.path.name().unwrap_or("unknown").to_string();
        let brand = def.path.segments.join("::");

        let mut members: Vec<String> = Vec::new();

        members.push(format!("        private [\" \"]!: '{brand}';"));

        if let StructShape::Struct(fields) = &def.shape {
            for field in fields {
                let ty = self.type_to_ts(field.type_id);
                let mut member = String::new();
                if let Some(docs) = &field.docs {
                    member.push_str(&doc_comment(docs, "        "));
                }
                member.push_str(&format!("        {}: {ty};", field.name));
                members.push(member);
            }
        } else if let StructShape::Tuple(types) = &def.shape {
            for (i, &type_id) in types.iter().enumerate() {
                let ty = self.type_to_ts(type_id);
                members.push(format!("        {i}: {ty};"));
            }
        }

        let ctor = match &def.boundary {
            StructBoundary::Ref {
                ctor: Ctor::Explicit(fid),
            } => {
                let fid = *fid;
                let params: Vec<String> = self
                    .ir
                    .fn_(fid)
                    .params
                    .clone()
                    .iter()
                    .map(|p| {
                        let ty = self.type_to_ts(p.type_id);
                        format!("{}: {ty}", p.name)
                    })
                    .collect();
                format!("        constructor({});", params.join(", "))
            }
            _ => {
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
                .map(|(i, &type_id)| {
                    let ty = self.type_to_ts(type_id);
                    format!("_{i}: {ty}")
                })
                .collect(),
            StructShape::Struct(fields) => fields
                .iter()
                .map(|f| {
                    let ty = self.type_to_ts(f.type_id);
                    format!("{}: {ty}", f.name)
                })
                .collect(),
        }
    }

    fn print_fn(&mut self, fid: FnId) -> String {
        let def = self.ir.fn_(fid).clone();
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

        let ret = self.type_to_ts(def.return_type);
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

fn parens_if_union(s: String) -> String {
    if s.contains(" | ") {
        format!("({s})")
    } else {
        s
    }
}

fn mod_ambient_path(mod_: &super::ModDef) -> String {
    format!("$rust/{}", mod_.path.segments.join("/"))
}

fn is_canonical_origin(item_path: &ItemPath, mod_segments: &[String]) -> bool {
    if item_path.segments.len() < 2 {
        return false;
    }
    &item_path.segments[..item_path.segments.len() - 1] == mod_segments
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
