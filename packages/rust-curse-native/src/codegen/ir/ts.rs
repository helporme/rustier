use std::collections::{BTreeMap, BTreeSet, HashSet};

use super::{
    Ctor, EnumId, Export, FnId, FnParam, ItemPath, ModId, NumberType, ParamMode, StructBoundary,
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
    local_enums: HashSet<EnumId>,
    imports: BTreeMap<String, BTreeSet<String>>,
    needs_copy: bool,
}

impl<'a> Printer<'a> {
    fn new(ir: &'a IR) -> Self {
        Self {
            ir,
            local_structs: HashSet::new(),
            local_enums: HashSet::new(),
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
        self.local_enums.clear();
        let exports = self.ir.mod_(mid).exports.clone();
        for export in &exports {
            match export {
                Export::Struct(sid) => {
                    self.local_structs.insert(*sid);
                }
                Export::Enum(eid) => {
                    self.local_enums.insert(*eid);
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
        let exports = self.ir.mod_(mid).exports.clone();
        let mut parts: Vec<String> = Vec::new();
        for export in &exports {
            match export {
                Export::Struct(sid) => parts.push(self.print_struct(*sid)),
                Export::Enum(eid) => parts.push(self.print_enum(*eid)),
                Export::Fn(fid) => parts.push(self.print_fn(*fid)),
                Export::Mod(_) => {}
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
            Type::Vec(inner)
            | Type::Slice(inner)
            | Type::HashSet(inner)
            | Type::BTreeSet(inner) => {
                let inner_ts = self.type_to_ts(inner);
                format!("{}[]", parens_if_union(inner_ts))
            }
            Type::Array(inner, _) => {
                let inner_ts = self.type_to_ts(inner);
                format!("{}[]", parens_if_union(inner_ts))
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
            Type::Enum(eid) => {
                let path = self.ir.enum_(eid).path.clone();
                let name = path.name().unwrap_or("unknown").to_string();
                if !self.local_enums.contains(&eid) {
                    self.record_import(&path);
                }
                name
            }
        }
    }

    fn param_to_ts(&mut self, param: &FnParam) -> String {
        let base = self.type_to_ts(param.type_id);
        let is_nominal = matches!(
            self.ir.type_(param.type_id),
            Type::Struct(_) | Type::Enum(_)
        );
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

        let mut members: Vec<String> = Vec::new();

        members.push(format!("        private [\" \"]!: '{name}';"));

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

    fn print_enum(&mut self, eid: EnumId) -> String {
        let def = self.ir.enum_(eid).clone();
        let name = def.path.name().unwrap_or("unknown").to_string();
        let mut out = String::new();

        if let Some(docs) = &def.docs {
            out.push_str(&doc_comment(docs, "    "));
        }

        let all_unit = def
            .variants
            .iter()
            .all(|v| matches!(v.shape, StructShape::Unit));
        let any_variant_docs = def.variants.iter().any(|v| v.docs.is_some());

        if all_unit && !any_variant_docs {
            let variants: Vec<String> = def
                .variants
                .iter()
                .map(|v| format!("'{}'", v.name))
                .collect();
            out.push_str(&format!(
                "    export type {name} = {};",
                variants.join(" | ")
            ));
        } else if all_unit {
            // JSDoc doesn't support per-literal documentation, so use // comments.
            let mut lines: Vec<String> = Vec::new();
            for v in &def.variants {
                let mut line = String::new();
                if let Some(docs) = &v.docs {
                    line.push_str(&line_comment(docs, "        "));
                }
                line.push_str(&format!("        | '{}'", v.name));
                lines.push(line);
            }
            out.push_str(&format!("    export type {name} =\n"));
            out.push_str(&lines.join("\n"));
            out.push(';');
        } else {
            let has_type_field = def.variants.iter().any(|v| {
                if let StructShape::Struct(fields) = &v.shape {
                    fields.iter().any(|f| f.name == "type")
                } else {
                    false
                }
            });
            let disc = if has_type_field { "$type" } else { "type" };

            let mut variant_lines: Vec<String> = Vec::new();
            for v in &def.variants {
                let mut field_lines: Vec<String> = Vec::new();

                // Discriminant field with optional doc comment above it.
                if let Some(docs) = &v.docs {
                    field_lines.push(format!("            /** {} */", docs.trim()));
                }
                field_lines.push(format!("            {disc}: '{}';", v.name));

                match &v.shape {
                    StructShape::Unit => {}
                    StructShape::Tuple(types) => {
                        for (i, &type_id) in types.iter().enumerate() {
                            let ty = self.type_to_ts(type_id);
                            field_lines.push(format!("            _{i}: {ty};"));
                        }
                    }
                    StructShape::Struct(fields) => {
                        for field in fields {
                            let ty = self.type_to_ts(field.type_id);
                            field_lines.push(format!("            {}: {ty};", field.name));
                        }
                    }
                }

                variant_lines.push(format!(
                    "        | {{\n{}\n        }}",
                    field_lines.join("\n")
                ));
            }
            out.push_str(&format!("    export type {name} =\n"));
            out.push_str(&variant_lines.join("\n"));
            out.push(';');
        }

        out
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

fn line_comment(docs: &str, indent: &str) -> String {
    docs.trim()
        .lines()
        .map(|l| format!("{indent}// {}\n", l.trim()))
        .collect()
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
