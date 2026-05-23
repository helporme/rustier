//! IR -> Rust wasm-bindgen glue printer.

use std::collections::HashSet;

use super::ir::{
    rust_path, ClassDef, ClassField, CtorDef, FnDef, FnParam, Ir, IrId, Item, ParamMode, Primitive,
    Receiver, StructExpr, Type,
};
use super::mangle::{self, is_canonical_origin};
use super::projection::PROJECTION_PREAMBLE;

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

    fn run(&self) -> String {
        let mut out = String::new();
        out.push_str("#![allow(non_camel_case_types, non_snake_case)]\n");
        out.push_str("use wasm_bindgen::prelude::*;\n");
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
            out.push('\n');
            out.push_str(&self.print_coercer(def.id));
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
            "#[wasm_bindgen(js_name = \"{mangled}\")]\npub struct {mangled} {{\n"
        ));
        out.push_str(&format!(
            "    {root}: Rc<RefCell<Rc<dyn __cursed::Projection<{orig}>>>>,\n"
        ));
        for field in &def.fields {
            if let Type::Class(child) = &field.ty {
                let child_orig = self.class_orig(*child);
                out.push_str(&format!(
                    "    {}: Option<Weak<RefCell<Rc<dyn __cursed::Projection<{child_orig}>>>>>,\n",
                    mangle::cache_field(&field.orig_name),
                ));
            }
        }
        out.push_str("}\n\n");

        out.push_str(&format!("#[wasm_bindgen]\nimpl {mangled} {{\n"));
        if let Some(ctor) = &def.ctor {
            out.push_str(&self.print_ctor(&def, ctor, &orig));
        }
        if let Some(se) = &def.struct_expr {
            if def.ctor.is_some() {
                out.push('\n');
            }
            out.push_str(&self.print_of(&def, se, &orig));
        }
        for field in &def.fields {
            out.push('\n');
            out.push_str(&self.print_getter_setter(field));
        }
        for &mid in &def.methods {
            let fn_def = self.ir.get_fn_unchecked(mid).clone();
            out.push('\n');
            out.push_str(&self.print_method(&fn_def, &orig));
        }
        out.push_str("}\n");

        out
    }

    fn print_coercer(&self, cid: IrId) -> String {
        let def = self.ir.get_class_unchecked(cid).clone();
        let orig = rust_path(&def.path, &self.ir.crate_name);
        let mangled = &def.mangled_name;
        let coerce = format!("__cursed_coerce_{mangled}");

        let mut out = String::new();
        out.push_str(&format!(
            "#[allow(dead_code)]\nfn {coerce}(val: JsValue) -> Result<__cursed::Coerced<{orig}>, JsValue> {{\n"
        ));

        // wasm-bindgen does not auto-impl `JsCast` for exported `pub struct`s
        // (only for `extern \"C\"` imports), so we can't `dyn_ref::<Self>()` to
        // peek inside an instance non-destructively. We rely on the fact that
        // `js_sys::Reflect::get` on a wasm-bindgen class instance dispatches
        // through the getter methods we emit, so it returns the same shape as
        // a plain object literal would. A class instance round-tripping
        // through `.of` thus produces an owned copy via the getters.
        if let Some(se) = &def.struct_expr {
            for f in &se.fields {
                let name = &f.name;
                match &f.ty {
                    Type::Primitive(prim) => {
                        out.push_str(&format!(
                            "    let __cursed_raw_{name} = js_sys::Reflect::get(&val, &JsValue::from_str(\"{name}\"))?;\n",
                        ));
                        out.push_str(&format!(
                            "    let __cursed_field_{name}: {ty} = {expr};\n",
                            ty = primitive_to_rs(prim),
                            expr =
                                primitive_from_jsvalue(prim, &format!("__cursed_raw_{name}"), name),
                        ));
                    }
                    Type::Class(child) => {
                        let child_coerce = format!(
                            "__cursed_coerce_{}",
                            self.ir.get_class_unchecked(*child).mangled_name
                        );
                        let child_orig = self.class_orig(*child);
                        out.push_str(&format!(
                            "    let __cursed_raw_{name} = js_sys::Reflect::get(&val, &JsValue::from_str(\"{name}\"))?;\n",
                        ));
                        out.push_str(&format!(
                            "    let __cursed_field_{name}: {child_orig} = {child_coerce}(__cursed_raw_{name})?.into_owned();\n",
                        ));
                    }
                }
            }
            let inits: Vec<String> = se
                .fields
                .iter()
                .map(|f| format!("{name}: __cursed_field_{name}", name = f.name))
                .collect();
            out.push_str(&format!(
                "    Ok(__cursed::Coerced::Owned({orig} {{ {} }}))\n",
                inits.join(", ")
            ));
        } else {
            out.push_str(&format!(
                "    let _ = val;\n    Err(JsValue::from_str(\"`{js}` cannot be constructed from JS: it has private fields or a non-clonable struct child\"))\n",
                js = def.js_name,
            ));
        }
        out.push_str("}\n");
        out
    }

    fn print_ctor(&self, def: &ClassDef, ctor: &CtorDef, orig: &str) -> String {
        let params = &ctor.params;

        let mut body = String::new();
        for p in params {
            if matches!(p.ty, Type::Class(_)) {
                body.push_str(&self.coerce_struct_arg(p));
            }
        }

        let args: Vec<String> = params
            .iter()
            .map(|p| {
                if matches!(p.ty, Type::Class(_)) {
                    format!("{}.into_owned()", co_local(&p.name))
                } else {
                    mangle::arg_name(&p.name)
                }
            })
            .collect();
        let built = format!("{orig}::new({})", args.join(", "));

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
            "    #[wasm_bindgen(constructor)]\n    pub fn new({}) -> Result<Self, JsValue> {{\n{body}        Ok(Self {{\n            {}: {root_init},\n{cache_inits}        }})\n    }}\n",
            self.render_ctor_params(params),
            mangle::ROOT_FIELD,
        )
    }

    fn print_of(&self, def: &ClassDef, _se: &StructExpr, orig: &str) -> String {
        let coerce = format!("__cursed_coerce_{}", def.mangled_name);
        let root_init = self.wrap_into_base_projection(orig, "__cursed_owned");
        let js_name = if def.of_escaped { "of$" } else { "of" };
        let rust_ident = "of";

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
            "    #[wasm_bindgen(js_name = \"{js_name}\")]\n    pub fn {rust_ident}(init: JsValue) -> Result<{cls}, JsValue> {{\n        let __cursed_owned: {orig} = {coerce}(init)?.into_owned();\n        Ok({cls} {{\n            {}: {root_init},\n{cache_inits}        }})\n    }}\n",
            mangle::ROOT_FIELD,
            cls = def.mangled_name,
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
                "    #[wasm_bindgen(getter, js_name = \"{js}\")]\n    pub fn {getter}(&self) -> {ty} {{\n        let __cursed_proj = self.{root}.borrow().clone();\n        let mut __cursed_out: Option<{ty}> = None;\n        __cursed_proj.with(&mut |__cursed_p| __cursed_out = Some(__cursed_p.{orig}{clone}));\n        __cursed_out.unwrap()\n    }}\n",
                getter = mangle::getter_fn(orig),
                root = mangle::ROOT_FIELD,
            ));
        }
        if field.has_setter {
            out.push_str(&format!(
                "    #[wasm_bindgen(setter, js_name = \"{js}\")]\n    pub fn {setter}(&self, {arg}: {ty}) {{\n        let __cursed_proj = self.{root}.borrow().clone();\n        let mut __cursed_arg = Some({arg});\n        __cursed_proj.with_mut(&mut |__cursed_p| __cursed_p.{orig} = __cursed_arg.take().unwrap());\n    }}\n",
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
                "    #[wasm_bindgen(getter, js_name = \"{js}\")]\n    pub fn {getter}(&mut self) -> {child_mangled} {{\n        if let Some(__cursed_cell) = self.{cache}.as_ref().and_then(Weak::upgrade) {{\n            return {child_lit};\n        }}\n        let __cursed_proj: Rc<dyn __cursed::Projection<{child_orig}>> = Rc::new(__cursed::Projector {{\n            parent: self.{root}.clone(),\n            project: |__cursed_p| &__cursed_p.{orig},\n            project_mut: |__cursed_p| &mut __cursed_p.{orig},\n        }});\n        let __cursed_cell = Rc::new(RefCell::new(__cursed_proj));\n        self.{cache} = Some(Rc::downgrade(&__cursed_cell));\n        {child_lit}\n    }}\n",
                getter = mangle::getter_fn(orig),
            ));
        }
        if field.has_setter {
            let arg = mangle::arg_name(orig);
            out.push_str(&format!(
                "    #[wasm_bindgen(setter, js_name = \"{js}\")]\n    pub fn {setter}(&mut self, {arg}: &{child_mangled}) {{\n        let mut __cursed_new_val: Option<{child_orig}> = None;\n        {arg}.{root}.borrow().with(&mut |__cursed_p| __cursed_new_val = Some(__cursed_p.clone()));\n        if let Some(__cursed_cell) = self.{cache}.as_ref().and_then(Weak::upgrade) {{\n            let __cursed_proj = __cursed_cell.borrow().clone();\n            let mut __cursed_snap: Option<{child_orig}> = None;\n            __cursed_proj.with(&mut |__cursed_p| __cursed_snap = Some(__cursed_p.clone()));\n            *__cursed_cell.borrow_mut() = Rc::new(RefCell::new(__cursed_snap.unwrap())) as Rc<dyn __cursed::Projection<{child_orig}>>;\n            self.{cache} = None;\n        }}\n        let __cursed_parent = self.{root}.borrow().clone();\n        __cursed_parent.with_mut(&mut |__cursed_p| {{\n            __cursed_p.{orig} = __cursed_new_val.take().unwrap();\n        }});\n    }}\n",
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

        let mut pre = String::new();
        for p in &fn_def.params {
            match (&p.ty, &p.mode) {
                (Type::Class(child), _) => {
                    let coerce = format!(
                        "__cursed_coerce_{}",
                        self.ir.get_class_unchecked(*child).mangled_name
                    );
                    let mut_ = if matches!(p.mode, ParamMode::ByMut) {
                        "mut "
                    } else {
                        ""
                    };
                    pre.push_str(&format!(
                        "    let {mut_}{co} = {coerce}({name})?;\n",
                        co = co_local(&p.name),
                        name = p.name,
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

        for p in &fn_def.params {
            if matches!(p.ty, Type::Class(_)) && matches!(p.mode, ParamMode::ByValue) {
                pre.push_str(&format!(
                    "    let {val} = {co}.into_owned();\n",
                    val = val_local(&p.name),
                    co = co_local(&p.name),
                ));
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
                body.push_str(&format!("    {call};\n    Ok(())\n"));
            } else {
                body.push_str(&format!(
                    "    Ok({})\n",
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
                    "{co}.{method}(&mut |__cursed_{name}| {{\n{nested}\n}});",
                    co = co_local(name),
                );
            }
            body.push_str(&format!("    {nested}\n"));
            if !is_unit {
                body.push_str("    let __cursed_ret_val = __cursed_ret.unwrap();\n");
                body.push_str(&format!(
                    "    Ok({})\n",
                    self.wrap_return_tail(&fn_def.return_type, "__cursed_ret_val"),
                ));
            } else {
                body.push_str("    Ok(())\n");
            }
        }

        let mut out = String::new();
        out.push_str(&rust_doc(&format!("fn glue for `{orig_path}`"), ""));
        if let Some(docs) = &fn_def.docs {
            out.push_str(&rust_doc(docs, ""));
        }
        out.push_str(&format!("#[wasm_bindgen(js_name = \"{glue}\")]\n"));
        let params_sig = self.render_fn_params(&fn_def.params);
        if is_unit {
            out.push_str(&format!(
                "pub fn {glue}({params_sig}) -> Result<(), JsValue> {{\n{body}}}\n"
            ));
        } else {
            let ret_ty = self.type_to_rs(&fn_def.return_type);
            out.push_str(&format!(
                "pub fn {glue}({params_sig}) -> Result<{ret_ty}, JsValue> {{\n{body}}}\n"
            ));
        }
        out
    }

    /// Emit a method (instance or static) inside a class wrapper's
    /// `#[wasm_bindgen] impl` block.
    fn print_method(&self, fn_def: &FnDef, class_orig: &str) -> String {
        let receiver = fn_def.receiver;
        let js_name = &fn_def.js_name;
        let method_name = fn_def.path.name().unwrap_or("anon");
        let method_ident = mangle::method_fn(method_name);
        // UFCS: rustdoc loses the impl context in `path`, so it would render
        // as just the method name. Reach back to the class's canonical path
        // so the user fn resolves unambiguously across modules and impls.
        let user_path = format!("{class_orig}::{method_name}");
        let is_unit = matches!(fn_def.return_type, Type::Primitive(Primitive::Unit));

        let self_sig = match receiver {
            None => "".to_string(),
            Some(Receiver::ByRef) | Some(Receiver::ByValue) => "&self".to_string(),
            Some(Receiver::ByMut) => "&mut self".to_string(),
        };

        let mut pre = String::new();
        match receiver {
            None => {}
            Some(Receiver::ByRef) | Some(Receiver::ByMut) => {
                pre.push_str(&format!(
                    "        let __cursed_proj_self = self.{root}.borrow().clone();\n",
                    root = mangle::ROOT_FIELD,
                ));
            }
            Some(Receiver::ByValue) => {
                pre.push_str(&format!(
                    "        let __cursed_self_val: {class_orig} = {{\n            let __cursed_proj = self.{root}.borrow().clone();\n            let mut __cursed_out: Option<{class_orig}> = None;\n            __cursed_proj.with(&mut |__cursed_p| __cursed_out = Some(__cursed_p.clone()));\n            __cursed_out.unwrap()\n        }};\n",
                    root = mangle::ROOT_FIELD,
                ));
            }
        }

        for p in &fn_def.params {
            match (&p.ty, &p.mode) {
                (Type::Class(child), _) => {
                    let coerce = format!(
                        "__cursed_coerce_{}",
                        self.ir.get_class_unchecked(*child).mangled_name
                    );
                    let mut_ = if matches!(p.mode, ParamMode::ByMut) {
                        "mut "
                    } else {
                        ""
                    };
                    pre.push_str(&format!(
                        "        let {mut_}{co} = {coerce}({name})?;\n",
                        co = co_local(&p.name),
                        name = p.name,
                    ));
                }
                _ if p.needs_stash => {
                    pre.push_str(&format!(
                        "        let mut {owned}: Option<{ty}> = Some({name});\n",
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
        for p in &fn_def.params {
            if matches!(p.ty, Type::Class(_)) && matches!(p.mode, ParamMode::ByValue) {
                pre.push_str(&format!(
                    "        let {val} = {co}.into_owned();\n",
                    val = val_local(&p.name),
                    co = co_local(&p.name),
                ));
            }
        }

        let mut call_args: Vec<String> = Vec::new();
        match receiver {
            None => {}
            Some(Receiver::ByRef) | Some(Receiver::ByMut) => {
                call_args.push("__cursed_self".to_string());
            }
            Some(Receiver::ByValue) => {
                call_args.push("__cursed_self_val".to_string());
            }
        }
        for p in &fn_def.params {
            call_args.push(self.param_call_expr(p));
        }
        let call = format!("{user_path}({})", call_args.join(", "));

        let mut body = pre;
        if !fn_def.needs_closure_scope {
            if is_unit {
                body.push_str(&format!("        {call};\n        Ok(())\n"));
            } else {
                body.push_str(&format!(
                    "        Ok({})\n",
                    self.wrap_return_tail(&fn_def.return_type, &call)
                ));
            }
        } else {
            if !is_unit {
                body.push_str(&format!(
                    "        let mut __cursed_ret: Option<{}> = None;\n",
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
                    "{co}.{method}(&mut |__cursed_{name}| {{\n{nested}\n}});",
                    co = co_local(name),
                );
            }
            match receiver {
                Some(Receiver::ByRef) => {
                    nested =
                        format!("__cursed_proj_self.with(&mut |__cursed_self| {{\n{nested}\n}});");
                }
                Some(Receiver::ByMut) => {
                    nested = format!(
                        "__cursed_proj_self.with_mut(&mut |__cursed_self| {{\n{nested}\n}});"
                    );
                }
                _ => {}
            }
            body.push_str(&format!("        {nested}\n"));
            if !is_unit {
                body.push_str("        let __cursed_ret_val = __cursed_ret.unwrap();\n");
                body.push_str(&format!(
                    "        Ok({})\n",
                    self.wrap_return_tail(&fn_def.return_type, "__cursed_ret_val"),
                ));
            } else {
                body.push_str("        Ok(())\n");
            }
        }

        let params_sig = self.render_fn_params(&fn_def.params);
        let head = match (self_sig.is_empty(), params_sig.is_empty()) {
            (true, _) => params_sig,
            (false, true) => self_sig,
            (false, false) => format!("{self_sig}, {params_sig}"),
        };

        let mut out = String::new();
        if let Some(docs) = &fn_def.docs {
            out.push_str(&rust_doc(docs, "    "));
        }
        out.push_str(&format!("    #[wasm_bindgen(js_name = \"{js_name}\")]\n"));
        if is_unit {
            out.push_str(&format!(
                "    pub fn {method_ident}({head}) -> Result<(), JsValue> {{\n{body}    }}\n"
            ));
        } else {
            let ret_ty = self.type_to_rs(&fn_def.return_type);
            out.push_str(&format!(
                "    pub fn {method_ident}({head}) -> Result<{ret_ty}, JsValue> {{\n{body}    }}\n"
            ));
        }
        out
    }

    fn wrap_into_base_projection(&self, orig_ty: &str, value_expr: &str) -> String {
        format!(
            "Rc::new(RefCell::new(Rc::new(RefCell::new({value_expr})) as Rc<dyn __cursed::Projection<{orig_ty}>>))"
        )
    }

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

    fn coerce_struct_arg(&self, p: &FnParam) -> String {
        let Type::Class(child) = &p.ty else {
            unreachable!("coerce_struct_arg called on non-struct param")
        };
        let coerce = format!(
            "__cursed_coerce_{}",
            self.ir.get_class_unchecked(*child).mangled_name
        );
        format!(
            "        let {co} = {coerce}({arg})?;\n",
            co = co_local(&p.name),
            arg = mangle::arg_name(&p.name),
        )
    }

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
                format!("{}.clone()", val_local(&p.name))
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
                    self.ctor_param_type_to_rs(p)
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn render_fn_params(&self, params: &[FnParam]) -> String {
        params
            .iter()
            .map(|p| format!("{}: {}", p.name, self.fn_param_type_to_rs(p)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn ctor_param_type_to_rs(&self, p: &FnParam) -> String {
        if matches!(p.ty, Type::Class(_)) {
            return "JsValue".to_string();
        }
        self.type_to_rs(&p.ty)
    }

    fn fn_param_type_to_rs(&self, p: &FnParam) -> String {
        if matches!(p.ty, Type::Class(_)) {
            return "JsValue".to_string();
        }
        self.type_to_rs(&p.ty)
    }

    fn type_to_rs(&self, ty: &Type) -> String {
        match ty {
            Type::Primitive(p) => primitive_to_rs(p).to_string(),
            Type::Class(cid) => self.ir.get_class_unchecked(*cid).mangled_name.clone(),
        }
    }

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

fn co_local(name: &str) -> String {
    format!("__cursed_co_{name}")
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

/// Build an expression that converts `raw` (a `JsValue`) into the given
/// primitive Rust type, or produces an error tagged with the field name.
fn primitive_from_jsvalue(p: &Primitive, raw: &str, field: &str) -> String {
    let err = |what: &str| {
        format!(
            "JsValue::from_str(\"field `{field}`: expected {what}\")",
            field = field,
            what = what,
        )
    };
    match p {
        Primitive::Boolean => format!("{raw}.as_bool().ok_or_else(|| {})?", err("boolean")),
        Primitive::String | Primitive::Str | Primitive::Char => {
            format!("{raw}.as_string().ok_or_else(|| {})?", err("string"))
        }
        Primitive::f64 => format!("{raw}.as_f64().ok_or_else(|| {})?", err("number")),
        Primitive::f32 => format!("{raw}.as_f64().ok_or_else(|| {})? as f32", err("number")),
        Primitive::i8 => format!("{raw}.as_f64().ok_or_else(|| {})? as i8", err("number")),
        Primitive::u8 => format!("{raw}.as_f64().ok_or_else(|| {})? as u8", err("number")),
        Primitive::i16 => format!("{raw}.as_f64().ok_or_else(|| {})? as i16", err("number")),
        Primitive::u16 => format!("{raw}.as_f64().ok_or_else(|| {})? as u16", err("number")),
        Primitive::i32 => format!("{raw}.as_f64().ok_or_else(|| {})? as i32", err("number")),
        Primitive::u32 => format!("{raw}.as_f64().ok_or_else(|| {})? as u32", err("number")),
        Primitive::i64 => format!("{raw}.try_into().map_err(|_| {})?", err("bigint")),
        Primitive::u64 => format!("{raw}.try_into().map_err(|_| {})?", err("bigint")),
        Primitive::isize => format!("{raw}.as_f64().ok_or_else(|| {})? as isize", err("number")),
        Primitive::usize => format!("{raw}.as_f64().ok_or_else(|| {})? as usize", err("number")),
        Primitive::Unit => "()".to_string(),
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
