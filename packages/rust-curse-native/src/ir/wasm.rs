//! IR -> Rust wasm-bindgen glue printer.

use std::collections::HashSet;

use super::mangle::{is_canonical_origin, mangle_copy, mangle_fn, mangle_ref};
use super::{
    rust_path, Ctor, FnDef, FnParam, Id, Item, NumberType, ParamMode, StructDef, StructField,
    StructShape, Type, IR,
};

impl IR {
    /// Prints the IR as wasm-bindgen glue code, exposing items in the `exposed`
    /// set.
    pub fn print_wasm(&self, exposed: &HashSet<Id>) -> String {
        Printer::new(self, exposed).run()
    }
}

struct Printer<'a> {
    ir: &'a IR,
    exposed: &'a HashSet<Id>,
}

impl<'a> Printer<'a> {
    fn new(ir: &'a IR, exposed: &'a HashSet<Id>) -> Self {
        Self { ir, exposed }
    }

    fn run(&mut self) -> String {
        let mut out = String::new();
        out.push_str("use wasm_bindgen::prelude::*;\n");

        for i in 0..self.ir.items.len() {
            let id = Id(i as u32);
            if let Item::Struct(_) = self.ir.get_unchecked(id) {
                if !self.exposed.contains(&id) {
                    continue;
                }
                out.push('\n');
                out.push_str(&self.print_struct_wrappers(id));
            }
        }

        for i in 0..self.ir.items.len() {
            let mid = Id(i as u32);
            let Item::Mod(mod_def) = self.ir.get_unchecked(mid) else {
                continue;
            };
            let mod_segs = mod_def.path.segments.clone();
            for export in mod_def.exports.clone() {
                if !self.exposed.contains(&export.target) {
                    continue;
                }
                let Item::Fn(fn_def) = self.ir.get_unchecked(export.target) else {
                    continue;
                };
                let canonical = fn_def.path.clone();
                if is_canonical_origin(&canonical, &mod_segs) {
                    out.push('\n');
                    out.push_str(&self.print_fn_glue(export.target));
                }
            }
        }

        out
    }

    fn print_struct_wrappers(&mut self, sid: Id) -> String {
        let def = self.ir.get_struct_unchecked(sid).clone();
        let orig = rust_path(&def.path, &self.ir.crate_name);
        let mangled = mangle_ref(&def.path);

        let mut out = String::new();
        if let Some(docs) = &def.docs {
            out.push_str(&rust_doc(docs, ""));
        }
        out.push_str(&format!(
            "#[wasm_bindgen(js_name = \"{mangled}\")]\npub struct {mangled} {{\n    inner: {orig},\n}}\n\n"
        ));

        out.push_str(&format!("#[wasm_bindgen]\nimpl {mangled} {{\n"));
        out.push_str(&self.print_ctor(&def, &orig));
        if let StructShape::Struct(fields) = &def.shape {
            for field in fields {
                out.push('\n');
                out.push_str(&self.print_getter_setter(field));
            }
        }
        out.push_str("}\n\n");

        out.push_str(&format!(
            "impl From<{orig}> for {mangled} {{\n    fn from(v: {orig}) -> Self {{ Self {{ inner: v }} }}\n}}\n\n"
        ));
        out.push_str(&format!(
            "impl From<{mangled}> for {orig} {{\n    fn from(v: {mangled}) -> Self {{ v.inner }}\n}}\n"
        ));

        if def.has_copy_variant {
            out.push('\n');
            out.push_str(&self.print_copy_wrapper(&def, &orig));
        }

        out
    }

    fn print_copy_wrapper(&mut self, def: &StructDef, orig: &str) -> String {
        let copy_mangled = mangle_copy(&def.path);
        let mut out = String::new();
        out.push_str(&format!(
            "#[derive(serde::Deserialize)]\npub struct {copy_mangled} {{\n"
        ));
        match &def.shape {
            StructShape::Unit => {}
            StructShape::Tuple(ts) => {
                for (i, ty) in ts.iter().enumerate() {
                    let ty = self.type_to_rs(ty);
                    out.push_str(&format!(
                        "    #[serde(rename = \"{i}\")]\n    pub _{i}: {ty},\n"
                    ));
                }
            }
            StructShape::Struct(fs) => {
                for f in fs {
                    let ty = self.type_to_rs(&f.ty);
                    out.push_str(&format!("    pub {}: {ty},\n", f.name));
                }
            }
        }
        out.push_str("}\n\n");

        out.push_str(&format!(
            "impl {copy_mangled} {{\n    pub fn from_js(val: JsValue) -> Result<Self, JsValue> {{\n        serde_wasm_bindgen::from_value(val).map_err(Into::into)\n    }}\n}}\n\n"
        ));

        out.push_str(&format!(
            "impl From<{copy_mangled}> for {orig} {{\n    fn from(v: {copy_mangled}) -> Self {{\n"
        ));
        match &def.shape {
            StructShape::Unit => {
                out.push_str(&format!("        {orig}\n"));
            }
            StructShape::Tuple(ts) => {
                let parts: Vec<String> = (0..ts.len()).map(|i| format!("v._{i}")).collect();
                out.push_str(&format!("        {orig}({})\n", parts.join(", ")));
            }
            StructShape::Struct(fs) => {
                let parts: Vec<String> = fs
                    .iter()
                    .map(|f| format!("{}: v.{}", f.name, f.name))
                    .collect();
                out.push_str(&format!("        {orig} {{ {} }}\n", parts.join(", ")));
            }
        }
        out.push_str("    }\n}\n");
        out
    }

    fn print_ctor(&mut self, def: &StructDef, orig: &str) -> String {
        match &def.ctor {
            Ctor::Explicit(fid) => {
                let fn_def = self.ir.get_fn_unchecked(*fid).clone();
                let params: Vec<String> = fn_def
                    .params
                    .iter()
                    .map(|p| {
                        let ty = self.type_to_rs(&p.ty);
                        format!("{}: {ty}", p.name)
                    })
                    .collect();
                let args: Vec<String> = fn_def.params.iter().map(|p| p.name.clone()).collect();
                format!(
                    "    #[wasm_bindgen(constructor)]\n    pub fn new({}) -> Self {{\n        Self {{ inner: {orig}::new({}) }}\n    }}\n",
                    params.join(", "),
                    args.join(", "),
                )
            }
            Ctor::Implicit => {
                let (params, body) = match &def.shape {
                    StructShape::Unit => (vec![], orig.to_string()),
                    StructShape::Tuple(ts) => {
                        let params: Vec<String> = ts
                            .iter()
                            .enumerate()
                            .map(|(i, ty)| {
                                let ty = self.type_to_rs(ty);
                                format!("_{i}: {ty}")
                            })
                            .collect();
                        let args: Vec<String> = (0..ts.len()).map(|i| format!("_{i}")).collect();
                        (params, format!("{orig}({})", args.join(", ")))
                    }
                    StructShape::Struct(fs) => {
                        let params: Vec<String> = fs
                            .iter()
                            .map(|f| {
                                let ty = self.type_to_rs(&f.ty);
                                format!("{}: {ty}", f.name)
                            })
                            .collect();
                        let names: Vec<String> = fs.iter().map(|f| f.name.clone()).collect();
                        (params, format!("{orig} {{ {} }}", names.join(", ")))
                    }
                };
                format!(
                    "    #[wasm_bindgen(constructor)]\n    pub fn new({}) -> Self {{\n        Self {{ inner: {body} }}\n    }}\n",
                    params.join(", ")
                )
            }
        }
    }

    fn print_getter_setter(&mut self, field: &StructField) -> String {
        let name = &field.name;
        let ty = self.type_to_rs(&field.ty);
        let needs_clone = !is_primitive_ty(&field.ty);
        let getter_body = if needs_clone {
            format!("self.inner.{name}.clone()")
        } else {
            format!("self.inner.{name}")
        };
        format!(
            "    #[wasm_bindgen(getter, js_name = \"{name}\")]\n    pub fn {name}(&self) -> {ty} {{ {getter_body} }}\n    #[wasm_bindgen(setter, js_name = \"{name}\")]\n    pub fn set_{name}(&mut self, v: {ty}) {{ self.inner.{name} = v; }}\n"
        )
    }

    fn print_fn_glue(&mut self, fid: Id) -> String {
        let fn_def = self.ir.get_fn_unchecked(fid).clone();
        let glue_name = mangle_fn(&fn_def.path);
        let js_name = glue_name.clone();

        let orig_call_path = rust_path(&fn_def.path, &self.ir.crate_name);

        let params: Vec<String> = fn_def
            .params
            .iter()
            .map(|p| {
                let ty = self.param_type_to_rs(p);
                format!("{}: {ty}", p.name)
            })
            .collect();

        let call_args: Vec<String> = fn_def
            .params
            .iter()
            .map(|p| self.param_call_expr(p))
            .collect();

        let call = format!("{orig_call_path}({})", call_args.join(", "));
        let fallible = self.has_fallible_copy_param(&fn_def);
        let (ret_ty, body) = self.return_expr(&fn_def.return_type, &call, fallible);

        let mut out = String::new();
        if let Some(docs) = &fn_def.docs {
            out.push_str(&rust_doc(docs, ""));
        }
        out.push_str(&format!("#[wasm_bindgen(js_name = \"{js_name}\")]\n"));
        if ret_ty == "()" {
            out.push_str(&format!(
                "pub fn {glue_name}({}) {{\n    {body}\n}}\n",
                params.join(", ")
            ));
        } else {
            out.push_str(&format!(
                "pub fn {glue_name}({}) -> {ret_ty} {{\n    {body}\n}}\n",
                params.join(", ")
            ));
        }
        out
    }

    fn param_type_to_rs(&mut self, p: &FnParam) -> String {
        if let Type::Struct(sid) = &p.ty {
            let def = self.ir.get_struct_unchecked(*sid).clone();
            let handle = mangle_ref(&def.path);
            return match p.mode {
                ParamMode::ByRef => format!("&{handle}"),
                ParamMode::ByMut => format!("&mut {handle}"),
                ParamMode::ByValue => {
                    if def.has_copy_variant {
                        "JsValue".into()
                    } else {
                        handle
                    }
                }
            };
        }
        self.type_to_rs(&p.ty)
    }

    fn param_call_expr(&mut self, p: &FnParam) -> String {
        match (&p.ty, &p.mode) {
            (Type::Struct(_), ParamMode::ByRef) => format!("&{}.inner", p.name),
            (Type::Struct(_), ParamMode::ByMut) => format!("&mut {}.inner", p.name),
            (Type::Struct(sid), ParamMode::ByValue) => {
                let def = self.ir.get_struct_unchecked(*sid);
                if def.has_copy_variant {
                    let copy = mangle_copy(&def.path);
                    format!("{copy}::from_js({})?.into()", p.name)
                } else {
                    format!("{}.into()", p.name)
                }
            }
            (Type::Str, _) => format!("&{}", p.name),
            _ => p.name.clone(),
        }
    }

    fn return_expr(&mut self, ty: &Type, call: &str, fallible: bool) -> (String, String) {
        match ty {
            Type::Unit => {
                if fallible {
                    ("Result<(), JsValue>".into(), format!("{call};\n    Ok(())"))
                } else {
                    ("()".into(), format!("{call};"))
                }
            }
            Type::Struct(_) => {
                let ty = self.type_to_rs(ty);
                if fallible {
                    (
                        format!("Result<{ty}, JsValue>"),
                        format!("Ok({call}.into())"),
                    )
                } else {
                    (ty, format!("{call}.into()"))
                }
            }
            _ => {
                let ty = self.type_to_rs(ty);
                if fallible {
                    (format!("Result<{ty}, JsValue>"), format!("Ok({call})"))
                } else {
                    (ty, call.to_string())
                }
            }
        }
    }

    fn has_fallible_copy_param(&self, fn_def: &FnDef) -> bool {
        fn_def.params.iter().any(|p| {
            if !matches!(p.mode, ParamMode::ByValue) {
                return false;
            }
            if let Type::Struct(sid) = &p.ty {
                self.ir.get_struct_unchecked(*sid).has_copy_variant
            } else {
                false
            }
        })
    }

    fn type_to_rs(&mut self, ty: &Type) -> String {
        match ty {
            Type::Unit => "()".into(),
            Type::Boolean => "bool".into(),
            Type::Char => "char".into(),
            Type::String | Type::Str => "String".into(),
            Type::Number(n) => number_name(n).to_string(),
            Type::Struct(sid) => mangle_ref(&self.ir.get_struct_unchecked(*sid).path.clone()),
        }
    }
}

fn is_primitive_ty(t: &Type) -> bool {
    matches!(t, Type::Boolean | Type::Char | Type::Number(_))
}

fn number_name(n: &NumberType) -> &'static str {
    match n {
        NumberType::i8 => "i8",
        NumberType::u8 => "u8",
        NumberType::i16 => "i16",
        NumberType::u16 => "u16",
        NumberType::i32 => "i32",
        NumberType::u32 => "u32",
        NumberType::i64 => "i64",
        NumberType::u64 => "u64",
        NumberType::isize => "isize",
        NumberType::usize => "usize",
        NumberType::f32 => "f32",
        NumberType::f64 => "f64",
    }
}

fn rust_doc(docs: &str, indent: &str) -> String {
    let trimmed = docs.trim();
    let mut out = String::new();
    for line in trimmed.lines() {
        out.push_str(&format!("{indent}/// {}\n", line.trim()));
    }
    out
}
