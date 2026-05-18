//! IR -> Rust napi glue printer.

use std::collections::HashSet;

use super::ir::{
    rust_path, Ctor, FnParam, Ir, IrId, Item, ParamMode, Primitive, StructDef, StructField,
    StructShape, Type,
};
use super::mangle::{is_canonical_origin, mangle};

impl Ir {
    /// Prints the IR as napi glue code, exposing items in the `exposed`
    /// set.
    pub fn print_napi(&self, exposed: &HashSet<IrId>) -> String {
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
        out.push_str("use napi_derive::napi;\n");

        for i in 0..self.ir.items.len() {
            let id = IrId(i as u32);
            if let Item::Struct(_) = self.ir.get_unchecked(id) {
                if !self.exposed.contains(&id) {
                    continue;
                }
                out.push('\n');
                out.push_str(&self.print_struct_wrappers(id));
            }
        }

        for i in 0..self.ir.items.len() {
            let mid = IrId(i as u32);
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

    fn print_struct_wrappers(&mut self, sid: IrId) -> String {
        let def = self.ir.get_struct_unchecked(sid).clone();
        let orig = rust_path(&def.path, &self.ir.crate_name);
        let mangled = mangle(&def.path);

        let mut out = String::new();
        if let Some(docs) = &def.docs {
            out.push_str(&rust_doc(docs, ""));
        }
        out.push_str(&format!(
            "#[napi(js_name = \"{mangled}\")]\npub struct {mangled} {{\n    inner: {orig},\n}}\n\n"
        ));

        out.push_str(&format!("#[napi]\nimpl {mangled} {{\n"));
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
                    "    #[napi(constructor)]\n    pub fn new({}) -> Self {{\n        Self {{ inner: {orig}::new({}) }}\n    }}\n",
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
                    "    #[napi(constructor)]\n    pub fn new({}) -> Self {{\n        Self {{ inner: {body} }}\n    }}\n",
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
            "    #[napi(getter, js_name = \"{name}\")]\n    pub fn {name}(&self) -> {ty} {{ {getter_body} }}\n    #[napi(setter, js_name = \"{name}\")]\n    pub fn set_{name}(&mut self, v: {ty}) {{ self.inner.{name} = v; }}\n"
        )
    }

    fn print_fn_glue(&mut self, fid: IrId) -> String {
        let fn_def = self.ir.get_fn_unchecked(fid).clone();
        let glue_name = mangle(&fn_def.path);
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
        let (ret_ty, body) = self.return_expr(&fn_def.return_type, &call);

        let mut out = String::new();
        if let Some(docs) = &fn_def.docs {
            out.push_str(&rust_doc(docs, ""));
        }
        out.push_str(&format!("#[napi(js_name = \"{js_name}\")]\n"));
        if matches!(fn_def.return_type, Type::Primitive(Primitive::Unit)) {
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
            let handle = mangle(&def.path);
            return match p.mode {
                ParamMode::ByRef => format!("&{handle}"),
                ParamMode::ByMut => format!("&mut {handle}"),
                ParamMode::ByValue => handle,
            };
        }
        self.type_to_rs(&p.ty)
    }

    fn param_call_expr(&mut self, p: &FnParam) -> String {
        match (&p.ty, &p.mode) {
            (Type::Struct(_), ParamMode::ByRef) => format!("&{}.inner", p.name),
            (Type::Struct(_), ParamMode::ByMut) => format!("&mut {}.inner", p.name),
            (Type::Struct(_), ParamMode::ByValue) => format!("{}.into()", p.name),
            (Type::Primitive(Primitive::Str), _) => format!("&{}", p.name),
            _ => p.name.clone(),
        }
    }

    fn return_expr(&mut self, ty: &Type, call: &str) -> (String, String) {
        match ty {
            Type::Primitive(Primitive::Unit) => ("()".into(), format!("{call};")),
            Type::Struct(_) => {
                let ty = self.type_to_rs(ty);
                (ty, format!("{call}.into()"))
            }
            _ => {
                let ty = self.type_to_rs(ty);
                (ty, call.to_string())
            }
        }
    }

    fn type_to_rs(&mut self, ty: &Type) -> String {
        match ty {
            Type::Primitive(p) => primitive_to_rs(p).to_string(),
            Type::Struct(sid) => mangle(&self.ir.get_struct_unchecked(*sid).path.clone()),
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

fn is_primitive_ty(t: &Type) -> bool {
    matches!(
        t,
        Type::Primitive(
            Primitive::Boolean
                | Primitive::Char
                | Primitive::i8
                | Primitive::u8
                | Primitive::i16
                | Primitive::u16
                | Primitive::i32
                | Primitive::u32
                | Primitive::i64
                | Primitive::u64
                | Primitive::isize
                | Primitive::usize
                | Primitive::f32
                | Primitive::f64,
        )
    )
}

fn rust_doc(docs: &str, indent: &str) -> String {
    let trimmed = docs.trim();
    let mut out = String::new();
    for line in trimmed.lines() {
        out.push_str(&format!("{indent}/// {}\n", line.trim()));
    }
    out
}
