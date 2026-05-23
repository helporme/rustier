//! IR -> Rust napi glue printer.

use std::collections::HashSet;

use super::ir::{
    rust_path, ClassDef, ClassField, Ctor, FnParam, Ir, IrId, Item, ParamMode, Primitive, Type,
};
use super::mangle::{self, is_canonical_origin};
use super::projection::PROJECTION_PREAMBLE;

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

    fn run(&self) -> String {
        let mut out = String::new();
        out.push_str("use napi_derive::napi;\n");
        if self.ir.uses_projections {
            out.push_str(PROJECTION_PREAMBLE);
            out.push('\n');
        }

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

    fn print_class_wrappers(&self, cid: IrId) -> String {
        let def = self.ir.get_class_unchecked(cid).clone();
        let orig = rust_path(&def.path, &self.ir.crate_name);
        let mangled = &def.mangled_name;
        let root = mangle::ROOT_FIELD;

        let mut out = String::new();
        out.push_str(&rust_doc(&format!("class wrapper for `{orig}`"), ""));
        if let Some(docs) = &def.docs {
            out.push_str(&rust_doc(docs, ""));
        }

        out.push_str(&format!(
            "#[napi(js_name = \"{mangled}\")]\npub struct {mangled} {{\n"
        ));
        out.push_str(&format!(
            "    {root}: Rc<RefCell<Rc<dyn __cursed_Projection<{orig}>>>>,\n"
        ));
        for field in &def.fields {
            if let Type::Class(child) = &field.ty {
                let child_orig = self.class_orig(*child);
                out.push_str(&format!(
                    "    {}: Option<Weak<RefCell<Rc<dyn __cursed_Projection<{child_orig}>>>>>,\n",
                    mangle::cache_field(&field.orig_name),
                ));
            }
        }
        out.push_str("}\n\n");

        out.push_str(&format!("#[napi]\nimpl {mangled} {{\n"));
        out.push_str(&self.print_ctor(&def, &orig));
        for field in &def.fields {
            out.push('\n');
            out.push_str(&self.print_getter_setter(field));
        }
        out.push_str("}\n");

        out
    }

    fn print_ctor(&self, def: &ClassDef, orig: &str) -> String {
        let params = match &def.ctor {
            Ctor::Explicit { params } | Ctor::Implicit { params } => params,
        };

        // Snapshot every struct-typed arg into an owned value (the single
        // permitted move-in clone). Primitive args are consumed directly.
        let mut body = String::new();
        for p in params {
            if matches!(p.ty, Type::Class(_)) {
                body.push_str(&self.unwrap_struct_arg(p));
            }
        }

        let built = match &def.ctor {
            Ctor::Implicit { .. } => {
                let inits: Vec<String> = params
                    .iter()
                    .map(|p| {
                        let f = &p.name;
                        if matches!(p.ty, Type::Class(_)) {
                            format!("{f}: {}.take().unwrap()", val_local(f))
                        } else {
                            format!("{f}: {}", mangle::arg_name(f))
                        }
                    })
                    .collect();
                format!("{orig} {{ {} }}", inits.join(", "))
            }
            Ctor::Explicit { .. } => {
                let args: Vec<String> = params
                    .iter()
                    .map(|p| {
                        if matches!(p.ty, Type::Class(_)) {
                            format!("{}.take().unwrap()", val_local(&p.name))
                        } else {
                            mangle::arg_name(&p.name)
                        }
                    })
                    .collect();
                format!("{orig}::new({})", args.join(", "))
            }
        };

        let root_init = self.wrap_into_base_projection(orig, &built);

        let mut cache_inits = String::new();
        for field in &def.fields {
            if matches!(field.ty, Type::Class(_)) {
                cache_inits.push_str(&format!(
                    "            {}: None,\n",
                    mangle::cache_field(&field.orig_name)
                ));
            }
        }

        format!(
            "    #[napi(constructor)]\n    pub fn new({}) -> Self {{\n{body}        Self {{\n            {}: {root_init},\n{cache_inits}        }}\n    }}\n",
            self.render_ctor_params(params),
            mangle::ROOT_FIELD,
        )
    }

    fn print_getter_setter(&self, field: &ClassField) -> String {
        match &field.ty {
            Type::Class(child) => self.print_struct_getset(field, *child),
            Type::Primitive(_) => self.print_primitive_getset(field),
        }
    }

    fn print_primitive_getset(&self, field: &ClassField) -> String {
        let js = &field.js_name;
        let orig = &field.orig_name;
        let ty = self.type_to_rs(&field.ty);
        let arg = mangle::arg_name(orig);
        let clone = if field.clone_on_get { ".clone()" } else { "" };

        let mut out = String::new();
        if field.has_getter {
            out.push_str(&format!(
                "    #[napi(getter, js_name = \"{js}\")]\n    pub fn {getter}(&self) -> {ty} {{\n        let __cursed_proj = self.{root}.borrow().clone();\n        let mut __cursed_out: Option<{ty}> = None;\n        __cursed_proj.with(&mut |__cursed_p| __cursed_out = Some(__cursed_p.{orig}{clone}));\n        __cursed_out.unwrap()\n    }}\n",
                getter = mangle::getter_fn(orig),
                root = mangle::ROOT_FIELD,
            ));
        }
        if field.has_setter {
            // The assignment runs inside an `FnMut`, which can't move a
            // captured non-`Copy` value (e.g. `String`) out; `Option::take`
            // lifts it without re-cloning and stays uniform for `Copy` types.
            out.push_str(&format!(
                "    #[napi(setter, js_name = \"{js}\")]\n    pub fn {setter}(&self, {arg}: {ty}) {{\n        let __cursed_proj = self.{root}.borrow().clone();\n        let mut __cursed_arg = Some({arg});\n        __cursed_proj.with_mut(&mut |__cursed_p| __cursed_p.{orig} = __cursed_arg.take().unwrap());\n    }}\n",
                setter = mangle::setter_fn(orig),
                root = mangle::ROOT_FIELD,
            ));
        }
        out
    }

    fn print_struct_getset(&self, field: &ClassField, child: IrId) -> String {
        let js = &field.js_name;
        let orig = &field.orig_name;
        let child_mangled = self.ir.get_class_unchecked(child).mangled_name.clone();
        let child_orig = self.class_orig(child);
        let cache = mangle::cache_field(orig);
        let root = mangle::ROOT_FIELD;
        let child_lit = self.empty_wrapper_literal(child, "__cursed_cell");

        let mut out = String::new();
        if field.has_getter {
            out.push_str(&format!(
                "    #[napi(getter, js_name = \"{js}\")]\n    pub fn {getter}(&mut self) -> {child_mangled} {{\n        if let Some(__cursed_cell) = self.{cache}.as_ref().and_then(Weak::upgrade) {{\n            return {child_lit};\n        }}\n        let __cursed_proj: Rc<dyn __cursed_Projection<{child_orig}>> = Rc::new(__cursed_Projector {{\n            parent: self.{root}.clone(),\n            project: |__cursed_p| &__cursed_p.{orig},\n            project_mut: |__cursed_p| &mut __cursed_p.{orig},\n        }});\n        let __cursed_cell = Rc::new(RefCell::new(__cursed_proj));\n        self.{cache} = Some(Rc::downgrade(&__cursed_cell));\n        {child_lit}\n    }}\n",
                getter = mangle::getter_fn(orig),
            ));
        }
        if field.has_setter {
            let arg = mangle::arg_name(orig);
            out.push_str(&format!(
                "    #[napi(setter, js_name = \"{js}\")]\n    pub fn {setter}(&mut self, {arg}: &{child_mangled}) {{\n        let mut __cursed_new_val: Option<{child_orig}> = None;\n        {arg}.{root}.borrow().with(&mut |__cursed_p| __cursed_new_val = Some(__cursed_p.clone()));\n        if let Some(__cursed_cell) = self.{cache}.as_ref().and_then(Weak::upgrade) {{\n            let __cursed_proj = __cursed_cell.borrow().clone();\n            let mut __cursed_snap: Option<{child_orig}> = None;\n            __cursed_proj.with(&mut |__cursed_p| __cursed_snap = Some(__cursed_p.clone()));\n            *__cursed_cell.borrow_mut() = Rc::new(RefCell::new(__cursed_snap.unwrap())) as Rc<dyn __cursed_Projection<{child_orig}>>;\n            self.{cache} = None;\n        }}\n        let __cursed_parent = self.{root}.borrow().clone();\n        __cursed_parent.with_mut(&mut |__cursed_p| {{\n            __cursed_p.{orig} = __cursed_new_val.take().unwrap();\n        }});\n    }}\n",
                setter = mangle::setter_fn(orig),
            ));
        }
        out
    }

    fn print_fn_glue(&self, fid: IrId) -> String {
        let fn_def = self.ir.get_fn_unchecked(fid).clone();
        let glue = &fn_def.mangled_name;
        let orig_path = rust_path(&fn_def.path, &self.ir.crate_name);
        let is_unit = matches!(fn_def.return_type, Type::Primitive(Primitive::Unit));

        // Outer-scope statements: by-value struct snapshots, by-ref/by-mut
        // projection handles (pointer clones only), and owned-string stashes.
        let mut pre = String::new();
        for p in &fn_def.params {
            match (&p.ty, &p.mode) {
                (Type::Class(child), ParamMode::ByValue) => {
                    let orig_ty = self.class_orig(*child);
                    pre.push_str(&format!(
                        "    let mut {val}: Option<{orig_ty}> = None;\n    {name}.{root}.borrow().with(&mut |__cursed_p| {val} = Some(__cursed_p.clone()));\n",
                        val = val_local(&p.name),
                        name = p.name,
                        root = mangle::ROOT_FIELD,
                    ));
                }
                (Type::Class(_), ParamMode::ByRef | ParamMode::ByMut) => {
                    pre.push_str(&format!(
                        "    let __cursed_proj_{name} = {name}.{root}.borrow().clone();\n",
                        name = p.name,
                        root = mangle::ROOT_FIELD,
                    ));
                }
                _ if p.needs_stash => {
                    pre.push_str(&format!(
                        "    let mut {owned}: Option<{ty}> = Some({name});\n",
                        owned = owned_local(&p.name),
                        ty = primitive_to_rs(match &p.ty {
                            Type::Primitive(prim) => prim,
                            _ => unreachable!("needs_stash is only set on primitive params"),
                        }),
                        name = p.name,
                    ));
                }
                _ => {}
            }
        }

        let call_args: Vec<String> = fn_def
            .params
            .iter()
            .map(|p| self.param_call_expr(p))
            .collect();
        let call = format!("{orig_path}({})", call_args.join(", "));

        let mut body = pre;
        if !fn_def.needs_closure_scope {
            if is_unit {
                body.push_str(&format!("    {call};\n"));
            } else {
                body.push_str(&format!(
                    "    {}\n",
                    self.wrap_return_tail(&fn_def.return_type, &call)
                ));
            }
        } else {
            if !is_unit {
                body.push_str(&format!(
                    "    let mut __cursed_ret: Option<{}> = None;\n",
                    self.type_to_rs_owned(&fn_def.return_type),
                ));
            }
            let mut nested = if is_unit {
                format!("{call};")
            } else {
                format!("__cursed_ret = Some({call});")
            };
            for p in fn_def.params.iter().rev() {
                let method = match (&p.ty, &p.mode) {
                    (Type::Class(_), ParamMode::ByRef) => "with",
                    (Type::Class(_), ParamMode::ByMut) => "with_mut",
                    _ => continue,
                };
                let name = &p.name;
                nested = format!(
                    "__cursed_proj_{name}.{method}(&mut |__cursed_{name}| {{\n{nested}\n}});"
                );
            }
            body.push_str(&format!("    {nested}\n"));
            if !is_unit {
                body.push_str("    let __cursed_ret_val = __cursed_ret.unwrap();\n");
                body.push_str(&format!(
                    "    {}\n",
                    self.wrap_return_tail(&fn_def.return_type, "__cursed_ret_val"),
                ));
            }
        }

        let mut out = String::new();
        out.push_str(&rust_doc(&format!("fn glue for `{orig_path}`"), ""));
        if let Some(docs) = &fn_def.docs {
            out.push_str(&rust_doc(docs, ""));
        }
        out.push_str(&format!("#[napi(js_name = \"{glue}\")]\n"));
        let params_sig = self.render_fn_params(&fn_def.params);
        if is_unit {
            out.push_str(&format!("pub fn {glue}({params_sig}) {{\n{body}}}\n"));
        } else {
            let ret_ty = self.type_to_rs(&fn_def.return_type);
            out.push_str(&format!(
                "pub fn {glue}({params_sig}) -> {ret_ty} {{\n{body}}}\n"
            ));
        }
        out
    }

    /// `Rc::new(RefCell::new(Rc::new(RefCell::new(<value>)) as Rc<dyn ...>))`.
    fn wrap_into_base_projection(&self, orig_ty: &str, value_expr: &str) -> String {
        format!(
            "Rc::new(RefCell::new(Rc::new(RefCell::new({value_expr})) as Rc<dyn __cursed_Projection<{orig_ty}>>))"
        )
    }

    /// A wrapper struct literal with `__cursed_root` set to `root_expr` and
    /// every cache slot initialized to `None`.
    fn empty_wrapper_literal(&self, cid: IrId, root_expr: &str) -> String {
        let def = self.ir.get_class_unchecked(cid);
        let mut s = format!(
            "{} {{ {}: {root_expr}",
            def.mangled_name,
            mangle::ROOT_FIELD
        );
        for field in &def.fields {
            if matches!(field.ty, Type::Class(_)) {
                s.push_str(&format!(
                    ", {}: None",
                    mangle::cache_field(&field.orig_name)
                ));
            }
        }
        s.push_str(" }");
        s
    }

    /// Snapshot a struct-typed arg into `Option<<orig>>` so step 2 can
    /// `.take()` it without re-cloning.
    fn unwrap_struct_arg(&self, p: &FnParam) -> String {
        let Type::Class(child) = &p.ty else {
            unreachable!("unwrap_struct_arg called on non-struct param")
        };
        let orig_ty = self.class_orig(*child);
        format!(
            "        let mut {val}: Option<{orig_ty}> = None;\n        {arg}.{root}.borrow().with(&mut |__cursed_p| {val} = Some(__cursed_p.clone()));\n",
            val = val_local(&p.name),
            arg = mangle::arg_name(&p.name),
            root = mangle::ROOT_FIELD,
        )
    }

    /// Return-position tail expression: a struct return is moved (not cloned)
    /// into a fresh base projection; a primitive return is bare.
    fn wrap_return_tail(&self, ty: &Type, value_expr: &str) -> String {
        match ty {
            Type::Class(cid) => {
                let orig = self.class_orig(*cid);
                let root = self.wrap_into_base_projection(&orig, value_expr);
                self.empty_wrapper_literal(*cid, &root)
            }
            _ => value_expr.to_string(),
        }
    }

    fn param_call_expr(&self, p: &FnParam) -> String {
        match (&p.ty, &p.mode) {
            (Type::Class(_), ParamMode::ByRef | ParamMode::ByMut) => format!("__cursed_{}", p.name),
            (Type::Class(_), ParamMode::ByValue) => {
                format!("{}.take().unwrap()", val_local(&p.name))
            }
            (Type::Primitive(Primitive::Str), _) => format!("&{}", p.name),
            _ if p.needs_stash => format!("{}.take().unwrap()", owned_local(&p.name)),
            _ => p.name.clone(),
        }
    }

    fn render_ctor_params(&self, params: &[FnParam]) -> String {
        params
            .iter()
            .map(|p| {
                format!(
                    "{}: {}",
                    mangle::arg_name(&p.name),
                    self.param_type_to_rs(p)
                )
            })
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
            // napi/wasm-bindgen classes can't cross the FFI boundary by value,
            // only by reference. A by-value param is borrowed here and its
            // underlying value is snapshotted (cloned) in the body.
            return match p.mode {
                ParamMode::ByMut => format!("&mut {handle}"),
                ParamMode::ByRef | ParamMode::ByValue => format!("&{handle}"),
            };
        }
        self.type_to_rs(&p.ty)
    }

    /// Wrapper / primitive type as it appears on the FFI signature.
    fn type_to_rs(&self, ty: &Type) -> String {
        match ty {
            Type::Primitive(p) => primitive_to_rs(p).to_string(),
            Type::Class(cid) => self.ir.get_class_unchecked(*cid).mangled_name.clone(),
        }
    }

    /// Owned underlying type used for the `__cursed_ret` capture slot: the
    /// original struct path for a class, the primitive otherwise.
    fn type_to_rs_owned(&self, ty: &Type) -> String {
        match ty {
            Type::Class(cid) => self.class_orig(*cid),
            Type::Primitive(p) => primitive_to_rs(p).to_string(),
        }
    }

    fn class_orig(&self, cid: IrId) -> String {
        rust_path(&self.ir.get_class_unchecked(cid).path, &self.ir.crate_name)
    }
}

fn val_local(name: &str) -> String {
    format!("__cursed_{name}_val")
}

fn owned_local(name: &str) -> String {
    format!("__cursed_{name}_owned")
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
