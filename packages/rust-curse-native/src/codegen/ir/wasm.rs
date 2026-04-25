//! IR -> Rust wasm-bindgen glue printer.
//!
//! Emits a single `.rs` source string containing (in order):
//! 1. A preamble `use wasm_bindgen::prelude::*;`.
//! 2. `__Ref` opaque handle wrappers (always) and optional `__Copy`
//!    plain-object wrappers (when `StructDef::has_copy_variant`) for
//!    every struct, plus `From` impls in both directions. User structs
//!    are referenced by their full `crate::...` path so no imports of
//!    user items are emitted.
//! 3. `#[wasm_bindgen]` glue functions for every canonical-origin fn
//!    export, delegating to the user's original fn via its full
//!    `crate::...` path.
//!
//! The emitted Rust is designed so that when `wasm-bindgen`
//! macro-expands this file the generated JS matches the shape produced
//! by the sibling `print_ts` printer.
//!
//! Unlike napi-rs, wasm-bindgen has no `#[wasm_bindgen(object)]`
//! equivalent for plain-object params. The `__Copy` wrapper is a pure
//! `serde::Deserialize` struct; glue fns that take an owned Copy struct
//! accept `JsValue` and call `__Copy::from_js(p)?`, which couples them
//! to a `Result<_, JsValue>` return.

use super::{
    rust_path, Ctor, ExportKind, FnDef, FnId, FnParam, ItemPath, ModId, NumberType, ParamMode,
    StructDef, StructField, StructId, StructShape, Type, TypeId, IR,
};

impl IR {
    pub fn print_wasm(&self) -> String {
        Printer::new(self).run()
    }
}

struct Printer<'a> {
    ir: &'a IR,
}

impl<'a> Printer<'a> {
    fn new(ir: &'a IR) -> Self {
        Self { ir }
    }

    fn run(&mut self) -> String {
        let mut out = String::new();
        out.push_str("#![allow(non_camel_case_types, non_snake_case)]\n");
        out.push_str("use wasm_bindgen::prelude::*;\n");

        for i in 0..self.ir.structs.len() {
            out.push('\n');
            out.push_str(&self.print_struct_wrappers(StructId(i as u32)));
        }

        for mi in 0..self.ir.mods.len() {
            let mid = ModId(mi as u32);
            let mod_segs = self.ir.mod_(mid).path.segments.clone();
            for export in self.ir.mod_(mid).exports.clone() {
                let ExportKind::Fn(fid) = export.kind else {
                    continue;
                };
                let canonical = self.ir.fn_(fid).path.clone();
                if is_canonical_origin(&canonical, &mod_segs) {
                    out.push('\n');
                    out.push_str(&self.print_fn_glue(fid, &mod_segs));
                }
            }
        }

        out
    }

    fn print_struct_wrappers(&mut self, sid: StructId) -> String {
        let def = self.ir.struct_(sid).clone();
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
        let copy_mangled = mangle_copy(&def.path.segments);
        let mut out = String::new();
        out.push_str(&format!(
            "#[derive(serde::Deserialize)]\npub struct {copy_mangled} {{\n"
        ));
        match &def.shape {
            StructShape::Unit => {}
            StructShape::Tuple(ts) => {
                for (i, &tid) in ts.iter().enumerate() {
                    let ty = self.type_to_rs(tid);
                    out.push_str(&format!(
                        "    #[serde(rename = \"{i}\")]\n    pub _{i}: {ty},\n"
                    ));
                }
            }
            StructShape::Struct(fs) => {
                for f in fs {
                    let ty = self.type_to_rs(f.type_id);
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
                let fn_def = self.ir.fn_(*fid).clone();
                let params: Vec<String> = fn_def
                    .params
                    .iter()
                    .map(|p| {
                        let ty = self.type_to_rs(p.type_id);
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
                            .map(|(i, &tid)| {
                                let ty = self.type_to_rs(tid);
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
                                let ty = self.type_to_rs(f.type_id);
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
        let ty = self.type_to_rs(field.type_id);
        let needs_clone = !is_primitive_ty(self.ir.type_(field.type_id));
        let getter_body = if needs_clone {
            format!("self.inner.{name}.clone()")
        } else {
            format!("self.inner.{name}")
        };
        format!(
            "    #[wasm_bindgen(getter, js_name = \"{name}\")]\n    pub fn {name}(&self) -> {ty} {{ {getter_body} }}\n    #[wasm_bindgen(setter, js_name = \"{name}\")]\n    pub fn set_{name}(&mut self, v: {ty}) {{ self.inner.{name} = v; }}\n"
        )
    }

    fn print_fn_glue(&mut self, fid: FnId, mod_segs: &[String]) -> String {
        let fn_def = self.ir.fn_(fid).clone();
        let fn_name = fn_def.path.name().unwrap().to_string();

        let mod_parts: &[String] = if mod_segs.len() > 1 {
            &mod_segs[1..]
        } else {
            &[]
        };
        let js_name = if mod_parts.is_empty() {
            fn_name.clone()
        } else {
            format!("{}__{}", mod_parts.join("__"), fn_name)
        };
        let glue_name = mangle_path(&fn_def.path.segments);

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
        let (ret_ty, body) = self.return_expr(fn_def.return_type, &call, fallible);

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
        if let Type::Struct(sid) = self.ir.type_(p.type_id).clone() {
            let def = self.ir.struct_(sid).clone();
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
        self.type_to_rs(p.type_id)
    }

    fn param_call_expr(&mut self, p: &FnParam) -> String {
        let ty = self.ir.type_(p.type_id).clone();
        match (&ty, &p.mode) {
            (Type::Struct(sid), ParamMode::ByRef) => {
                let _ = sid;
                format!("&{}.inner", p.name)
            }
            (Type::Struct(_), ParamMode::ByMut) => format!("&mut {}.inner", p.name),
            (Type::Struct(sid), ParamMode::ByValue) => {
                let def = self.ir.struct_(*sid);
                if def.has_copy_variant {
                    let copy = mangle_copy(&def.path.segments);
                    format!("{copy}::from_js({})?.into()", p.name)
                } else {
                    format!("{}.into()", p.name)
                }
            }
            (Type::Str, _) => format!("&{}", p.name),
            _ => p.name.clone(),
        }
    }

    fn return_expr(&mut self, tid: TypeId, call: &str, fallible: bool) -> (String, String) {
        match self.ir.type_(tid).clone() {
            Type::Unit => {
                if fallible {
                    ("Result<(), JsValue>".into(), format!("{call};\n    Ok(())"))
                } else {
                    ("()".into(), format!("{call};"))
                }
            }
            Type::Struct(_) => {
                let ty = self.type_to_rs(tid);
                if fallible {
                    (format!("Result<{ty}, JsValue>"), format!("Ok({call}.into())"))
                } else {
                    (ty, format!("{call}.into()"))
                }
            }
            _ => {
                let ty = self.type_to_rs(tid);
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
            if let Type::Struct(sid) = self.ir.type_(p.type_id) {
                self.ir.struct_(*sid).has_copy_variant
            } else {
                false
            }
        })
    }

    fn type_to_rs(&mut self, tid: TypeId) -> String {
        match self.ir.type_(tid).clone() {
            Type::Unit => "()".into(),
            Type::Boolean => "bool".into(),
            Type::Char => "char".into(),
            Type::String | Type::Str => "String".into(),
            Type::Number(n) => number_name(&n).to_string(),
            Type::Struct(sid) => mangle_ref(&self.ir.struct_(sid).path.clone()),
        }
    }
}

fn mangle_ref(path: &ItemPath) -> String {
    format!("__cursed_{}__Ref", path.segments.join("__"))
}

fn mangle_path(segments: &[String]) -> String {
    format!("__cursed_{}", segments.join("__"))
}

fn mangle_copy(segments: &[String]) -> String {
    format!("__cursed_{}__Copy", segments.join("__"))
}

fn is_canonical_origin(item_path: &ItemPath, mod_segments: &[String]) -> bool {
    if item_path.segments.len() < 2 {
        return false;
    }
    &item_path.segments[..item_path.segments.len() - 1] == mod_segments
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
