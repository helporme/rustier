//! IR -> Rust wasm-bindgen glue printer.

use std::collections::HashSet;

use super::ir::{
    rust_path, ClassDef, ClassField, Ctor, FnParam, Ir, IrId, Item, ParamMode, Primitive, Type,
};
use super::mangle::is_canonical_origin;

impl Ir {
    /// Prints the IR as wasm-bindgen glue code, exposing items in the `exposed`
    /// set.
    pub fn print_wasm(&self, exposed: &HashSet<IrId>) -> String {
        Printer::new(self, exposed).run()
    }
}

struct Printer<'a> {
    ir: &'a Ir,
    exposed: &'a HashSet<IrId>,
}

impl<'a> Printer<'a> {
    fn new(ir: &'a Ir, exposed: &'a HashSet<IrId>) -> Self {
        Self { ir, exposed }
    }

    fn run(&mut self) -> String {
        let mut out = String::new();
        out.push_str("use wasm_bindgen::prelude::*;\n");

        for i in 0..self.ir.items.len() {
            let Item::Class(def) = &self.ir.items[i] else {
                continue;
            };
            if !self.exposed.contains(&def.id) {
                continue;
            }
            out.push('\n');
            out.push_str(&self.print_class_wrappers(def.id));
        }

        for i in 0..self.ir.items.len() {
            let Item::Mod(mod_def) = &self.ir.items[i] else {
                continue;
            };
            let mod_segs = mod_def.path.segments.clone();
            for use_ in mod_def.uses.clone() {
                if !self.exposed.contains(&use_.target) {
                    continue;
                }
                let Item::Fn(fn_def) = self.ir.get_unchecked(use_.target) else {
                    continue;
                };
                let canonical = fn_def.path.clone();
                if is_canonical_origin(&canonical, &mod_segs) {
                    out.push('\n');
                    out.push_str(&self.print_fn_glue(use_.target));
                }
            }
        }

        out
    }

    fn print_class_wrappers(&mut self, cid: IrId) -> String {
        let def = self.ir.get_class_unchecked(cid).clone();
        let orig = rust_path(&def.path, &self.ir.crate_name);
        let mangled = &def.mangled_name;

        let mut out = String::new();
        if let Some(docs) = &def.docs {
            out.push_str(&rust_doc(docs, ""));
        }
        out.push_str(&format!(
            "#[wasm_bindgen(js_name = \"{mangled}\")]\npub struct {mangled} {{\n    inner: {orig},\n}}\n\n"
        ));

        out.push_str(&format!("#[wasm_bindgen]\nimpl {mangled} {{\n"));
        out.push_str(&self.print_ctor(&def, &orig));
        for field in &def.fields {
            out.push('\n');
            out.push_str(&self.print_getter_setter(field));
        }
        out.push_str("}\n\n");

        out.push_str(&format!(
            "impl From<{orig}> for {mangled} {{\n    fn from(v: {orig}) -> Self {{ Self {{ inner: v }} }}\n}}\n\n"
        ));
        out.push_str(&format!(
            "impl From<{mangled}> for {orig} {{\n    fn from(v: {mangled}) -> Self {{ v.inner }}\n}}\n"
        ));

        out
    }

    fn print_ctor(&self, def: &ClassDef, orig: &str) -> String {
        let (params, body) = match &def.ctor {
            Ctor::Explicit { params } => {
                let args: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                (params, format!("{orig}::new({})", args.join(", ")))
            }
            Ctor::Implicit { params } => {
                let names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                (params, format!("{orig} {{ {} }}", names.join(", ")))
            }
        };
        format!(
            "    #[wasm_bindgen(constructor)]\n    pub fn new({}) -> Self {{\n        Self {{ inner: {body} }}\n    }}\n",
            self.render_params(params),
        )
    }

    fn print_getter_setter(&self, field: &ClassField) -> String {
        let js = &field.js_name;
        let orig = &field.orig_name;
        let ty = self.type_to_rs(&field.ty);
        let mut out = String::new();
        if field.has_getter {
            let getter_body = if field.clone_on_get {
                format!("self.inner.{orig}.clone()")
            } else {
                format!("self.inner.{orig}")
            };
            out.push_str(&format!(
                "    #[wasm_bindgen(getter, js_name = \"{js}\")]\n    pub fn {orig}(&self) -> {ty} {{ {getter_body} }}\n"
            ));
        }
        if field.has_setter {
            out.push_str(&format!(
                "    #[wasm_bindgen(setter, js_name = \"{js}\")]\n    pub fn set_{orig}(&mut self, v: {ty}) {{ self.inner.{orig} = v; }}\n"
            ));
        }
        out
    }

    fn print_fn_glue(&mut self, fid: IrId) -> String {
        let fn_def = self.ir.get_fn_unchecked(fid).clone();
        let glue_name = &fn_def.mangled_name;

        let orig_call_path = rust_path(&fn_def.path, &self.ir.crate_name);

        let call_args: Vec<String> = fn_def
            .params
            .iter()
            .map(|p| self.param_call_expr(p))
            .collect();

        let call = format!("{orig_call_path}({})", call_args.join(", "));
        let (ret_ty, body) = self.return_expr(&fn_def.return_type, &call);

        let mut out = String::new();
        if let Some(docs) = &fn_def.docs {
            out.push_str(&rust_doc(docs, ""));
        }
        out.push_str(&format!("#[wasm_bindgen(js_name = \"{glue_name}\")]\n"));
        let params = self.render_fn_params(&fn_def.params);
        if ret_ty == "()" {
            out.push_str(&format!("pub fn {glue_name}({params}) {{\n    {body}\n}}\n"));
        } else {
            out.push_str(&format!(
                "pub fn {glue_name}({params}) -> {ret_ty} {{\n    {body}\n}}\n"
            ));
        }
        out
    }

    fn render_params(&self, params: &[FnParam]) -> String {
        params
            .iter()
            .map(|p| format!("{}: {}", p.name, self.type_to_rs(&p.ty)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn render_fn_params(&self, params: &[FnParam]) -> String {
        params
            .iter()
            .map(|p| format!("{}: {}", p.name, self.param_type_to_rs(p)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn param_type_to_rs(&self, p: &FnParam) -> String {
        if let Type::Class(cid) = &p.ty {
            let handle = self.ir.get_class_unchecked(*cid).mangled_name.clone();
            return match p.mode {
                ParamMode::ByRef => format!("&{handle}"),
                ParamMode::ByMut => format!("&mut {handle}"),
                ParamMode::ByValue => handle,
            };
        }
        self.type_to_rs(&p.ty)
    }

    fn param_call_expr(&self, p: &FnParam) -> String {
        match (&p.ty, &p.mode) {
            (Type::Class(_), ParamMode::ByRef) => format!("&{}.inner", p.name),
            (Type::Class(_), ParamMode::ByMut) => format!("&mut {}.inner", p.name),
            (Type::Class(_), ParamMode::ByValue) => format!("{}.into()", p.name),
            (Type::Primitive(Primitive::Str), _) => format!("&{}", p.name),
            _ => p.name.clone(),
        }
    }

    fn return_expr(&self, ty: &Type, call: &str) -> (String, String) {
        match ty {
            Type::Primitive(Primitive::Unit) => ("()".into(), format!("{call};")),
            Type::Class(_) => {
                let ty = self.type_to_rs(ty);
                (ty, format!("{call}.into()"))
            }
            _ => {
                let ty = self.type_to_rs(ty);
                (ty, call.to_string())
            }
        }
    }

    fn type_to_rs(&self, ty: &Type) -> String {
        match ty {
            Type::Primitive(p) => primitive_to_rs(p).to_string(),
            Type::Class(cid) => self.ir.get_class_unchecked(*cid).mangled_name.clone(),
        }
    }
}

fn primitive_to_rs(p: &Primitive) -> &'static str {
    match p {
        Primitive::Unit => "()",
        Primitive::Boolean => "bool",
        Primitive::Char => "char",
        Primitive::String | Primitive::Str => "String",
        Primitive::i8 => "i8",
        Primitive::u8 => "u8",
        Primitive::i16 => "i16",
        Primitive::u16 => "u16",
        Primitive::i32 => "i32",
        Primitive::u32 => "u32",
        Primitive::i64 => "i64",
        Primitive::u64 => "u64",
        Primitive::isize => "isize",
        Primitive::usize => "usize",
        Primitive::f32 => "f32",
        Primitive::f64 => "f64",
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
