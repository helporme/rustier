//! IR -> Rust napi glue printer.

use std::collections::HashSet;

use super::ir::{
    rust_path, ClassDef, ClassField, ClassShape, CtorDef, EnumVariantDef, FnDef, FnParam, Ir, IrId,
    Item, ParamMode, Primitive, Receiver, StructExpr, Type,
};
use super::mangle::{self, is_canonical_origin};
use super::projection::PROJECTION_PREAMBLE;
use crate::common::{coalesce_enum_fields, oxford_join, raw_ident, CoalescedField};

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
        out.push_str("#![allow(non_camel_case_types, non_snake_case)]\n");
        // `as _` brings the trait methods into scope without polluting the
        // namespace with names that could clash with user code.
        out.push_str("use napi::bindgen_prelude::{FromNapiValue as _, JsObjectValue as _};\n");
        out.push_str("use napi::JsValue as _;\n");
        out.push_str("use napi_derive::napi;\n");
        if self.ir.uses_projections {
            out.push_str(PROJECTION_PREAMBLE);
            out.push('\n');
            // Lets generated accessors propagate `StaleRef` with a bare `?`.
            out.push_str(
                "impl From<__cursed::StaleRef> for napi::Error {\n    fn from(__cursed_e: __cursed::StaleRef) -> Self {\n        napi::Error::from_reason(__cursed_e.message())\n    }\n}\n",
            );
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
        if matches!(
            self.ir.get_class_unchecked(cid).shape,
            ClassShape::Enum { .. }
        ) {
            return self.print_enum_wrappers(cid);
        }
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
            "    {root}: Rc<RefCell<Rc<dyn __cursed::Projection<{orig}>>>>,\n"
        ));
        for (slot, child_orig) in self.class_cache_slots(&def) {
            out.push_str(&format!(
                "    {slot}: Option<Weak<RefCell<Rc<dyn __cursed::Projection<{child_orig}>>>>>,\n",
            ));
        }
        out.push_str("}\n\n");

        out.push_str(&format!("#[napi]\nimpl {mangled} {{\n"));
        if let Some(ctor) = &def.ctor {
            out.push_str(&self.print_ctor(&def, ctor, &orig));
        }
        if let Some(se) = &def.struct_expr {
            if def.ctor.is_some() {
                out.push('\n');
            }
            out.push_str(&self.print_of(&def, se, &orig));
        }
        let ClassShape::Struct { fields } = &def.shape else {
            unreachable!("enum shape handled by print_enum_wrappers")
        };
        for field in fields {
            out.push('\n');
            out.push_str(&self.print_getter_setter(field));
        }
        for &mid in &def.methods {
            let fn_def = self.ir.get_fn_unchecked(mid).clone();
            out.push('\n');
            out.push_str(&self.print_method(def.id, &fn_def, &orig));
        }
        out.push_str("}\n");

        out
    }

    /// Per-struct runtime coercer. Free fn, lives next to the class wrapper.
    fn print_coercer(&self, cid: IrId) -> String {
        if matches!(
            self.ir.get_class_unchecked(cid).shape,
            ClassShape::Enum { .. }
        ) {
            return self.print_enum_coercer(cid);
        }
        let def = self.ir.get_class_unchecked(cid).clone();
        let orig = rust_path(&def.path, &self.ir.crate_name);
        let mangled = &def.mangled_name;
        let coerce = format!("__cursed_coerce_{mangled}");

        let mut out = String::new();
        out.push_str(&format!(
            "#[allow(dead_code)]\nfn {coerce}<'a>(val: napi::bindgen_prelude::Unknown<'a>) -> napi::Result<__cursed::Coerced<{orig}>> {{\n"
        ));
        // Instance-of fast path: try to unwrap as ClassInstance<__cursed_X>.
        // napi_unwrap fails for plain JS objects, so this acts as a check.
        out.push_str(&format!(
            "    let __cursed_env_raw = napi::JsValue::value(&val).env;\n    let __cursed_raw = napi::JsValue::raw(&val);\n    if let Ok(__cursed_inst) = unsafe {{ napi::bindgen_prelude::ClassInstance::<{mangled}>::from_napi_value(__cursed_env_raw, __cursed_raw) }} {{\n        return Ok(__cursed::Coerced::Projected(__cursed_inst.{root}.borrow().clone()));\n    }}\n",
            root = mangle::ROOT_FIELD,
        ));

        // Plain-object branch: only emitted when struct is struct-expressible.
        if let Some(se) = &def.struct_expr {
            out.push_str("    let __cursed_obj = val.coerce_to_object()?;\n");
            for f in &se.fields {
                let name = &f.name;
                match &f.ty {
                    Type::Primitive(_) => {
                        let ty = self.type_to_rs(&f.ty);
                        out.push_str(&format!(
                            "    let __cursed_field_{name}: {ty} = __cursed_obj.get_named_property(\"{name}\")?;\n",
                        ));
                    }
                    Type::Class(child) => {
                        let child_coerce = format!(
                            "__cursed_coerce_{}",
                            self.ir.get_class_unchecked(*child).mangled_name
                        );
                        let child_orig = self.class_orig(*child);
                        out.push_str(&format!(
                            "    let __cursed_field_{name}: {child_orig} = {child_coerce}(__cursed_obj.get_named_property::<napi::bindgen_prelude::Unknown>(\"{name}\")?)?.into_owned()?;\n",
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
            // Non-struct-expressible: reject plain objects with a clear error.
            out.push_str(&format!(
                "    Err(napi::Error::from_reason(format!(\n        \"expected an instance of class `{js}`; got a plain object, but `{js}` cannot be built from an object literal (private fields or non-clonable struct child)\"\n    )))\n",
                js = def.js_name,
            ));
        }
        out.push_str("}\n");
        out
    }

    // --- data enum wrappers ---

    fn print_enum_wrappers(&self, cid: IrId) -> String {
        let def = self.ir.get_class_unchecked(cid).clone();
        let orig = rust_path(&def.path, &self.ir.crate_name);
        let mangled = &def.mangled_name;
        let root = mangle::ROOT_FIELD;
        let ClassShape::Enum {
            variants,
            discriminator,
        } = &def.shape
        else {
            unreachable!("print_enum_wrappers on non-enum")
        };

        let mut out = String::new();
        out.push_str(&rust_doc(
            &format!("data-enum class wrapper for `{orig}`"),
            "",
        ));
        if let Some(docs) = &def.docs {
            out.push_str(&rust_doc(docs, ""));
        }
        let brand = def.path.segments.join("::");
        out.push_str(&format!(
            "#[napi(js_name = \"{mangled}\")]\npub struct {mangled} {{\n"
        ));
        out.push_str(&format!(
            "    {root}: Rc<RefCell<Rc<dyn __cursed::Projection<{orig}>>>>,\n"
        ));
        for (slot, child_orig) in self.class_cache_slots(&def) {
            out.push_str(&format!(
                "    {slot}: Option<Weak<RefCell<Rc<dyn __cursed::Projection<{child_orig}>>>>>,\n",
            ));
        }
        out.push_str("}\n\n");

        out.push_str(&format!("#[napi]\nimpl {mangled} {{\n"));
        if let Some(ctor) = &def.ctor {
            out.push_str(&self.print_ctor(&def, ctor, &orig));
        }
        if let Some(se) = &def.struct_expr {
            if def.ctor.is_some() {
                out.push('\n');
            }
            out.push_str(&self.print_of(&def, se, &orig));
        }
        out.push('\n');
        out.push_str(&self.print_enum_discriminator_getter(&orig, variants, discriminator));
        for cf in coalesce_enum_fields(variants) {
            out.push('\n');
            out.push_str(&self.print_enum_field_getset(&orig, &def.js_name, &brand, variants, &cf));
        }
        for &mid in &def.methods {
            let fn_def = self.ir.get_fn_unchecked(mid).clone();
            out.push('\n');
            out.push_str(&self.print_method(def.id, &fn_def, &orig));
        }
        out.push_str("}\n");
        out
    }

    /// `match <ident> { crate::E::Wasm { .. } => "wasm", ... }` producing the
    /// active tag as `&'static str`.
    fn enum_tag_match(&self, orig: &str, variants: &[EnumVariantDef], ident: &str) -> String {
        let mut arms = String::new();
        for v in variants {
            let pat = if v.is_unit {
                format!("{orig}::{}", v.rust_name)
            } else {
                format!("{orig}::{} {{ .. }}", v.rust_name)
            };
            arms.push_str(&format!("                {pat} => \"{}\",\n", v.tag));
        }
        format!("match {ident} {{\n{arms}            }}")
    }

    fn print_enum_discriminator_getter(
        &self,
        orig: &str,
        variants: &[EnumVariantDef],
        discriminator: &str,
    ) -> String {
        let match_expr = self.enum_tag_match(orig, variants, "__cursed_v");
        format!(
            "    #[napi(getter, js_name = \"{discriminator}\")]\n    pub fn __cursed_discriminator(&self) -> napi::Result<&'static str> {{\n        let __cursed_proj = self.{root}.borrow().clone();\n        let mut __cursed_out: &'static str = \"\";\n        __cursed_proj.with(&mut |__cursed_v| {{\n            __cursed_out = {match_expr};\n        }})?;\n        Ok(__cursed_out)\n    }}\n",
            root = mangle::ROOT_FIELD,
        )
    }

    fn print_enum_field_getset(
        &self,
        orig: &str,
        enum_js: &str,
        brand: &str,
        variants: &[EnumVariantDef],
        cf: &CoalescedField,
    ) -> String {
        if let Type::Class(child) = &cf.ty {
            return self.print_enum_class_field_getset(orig, enum_js, brand, variants, cf, *child);
        }
        let js = &cf.js_name;
        let ty = self.type_to_rs(&cf.ty);
        let root = mangle::ROOT_FIELD;

        // Getter: read out of every carrying variant, else `undefined`.
        let mut read_arms = String::new();
        for c in &cf.carriers {
            let bind = raw_ident(&c.orig_name);
            // `__cursed_v` is `&Enum`, so the field binds by reference: clone
            // it for owned non-`Copy` types, deref-copy for `Copy` primitives.
            let value = if c.clone_on_get {
                format!("{bind}.clone()")
            } else {
                format!("*{bind}")
            };
            read_arms.push_str(&format!(
                "            if let {orig}::{rust} {{ {bind}, .. }} = __cursed_v {{\n                __cursed_out = Some({value});\n            }}\n",
                rust = c.rust_name,
            ));
        }
        let mut out = String::new();
        out.push_str(&format!(
            "    #[napi(getter, js_name = \"{js}\")]\n    pub fn {getter}(&self) -> napi::Result<napi::bindgen_prelude::Either<{ty}, ()>> {{\n        let __cursed_proj = self.{root}.borrow().clone();\n        let mut __cursed_out: Option<{ty}> = None;\n        __cursed_proj.with(&mut |__cursed_v| {{\n{read_arms}        }})?;\n        Ok(match __cursed_out {{\n            Some(__cursed_val) => napi::bindgen_prelude::Either::A(__cursed_val),\n            None => napi::bindgen_prelude::Either::B(()),\n        }})\n    }}\n",
            getter = mangle::getter_fn(js),
        ));

        if cf.has_setter {
            let arg = mangle::arg_name(js);
            let mut write_arms = String::new();
            for c in &cf.carriers {
                let bind = raw_ident(&c.orig_name);
                write_arms.push_str(&format!(
                    "                {orig}::{rust} {{ {bind}, .. }} => {{\n                    *{bind} = __cursed_new.take().unwrap();\n                }}\n",
                    rust = c.rust_name,
                ));
            }
            // Only emit a wrong-variant catch-all when some variant lacks the
            // field (otherwise the arms are already exhaustive).
            let needs_catch_all = cf.carriers.len() < variants.len();
            let catch_all = if needs_catch_all {
                let tag_match = self.enum_tag_match(orig, variants, "__cursed_other");
                format!(
                    "                __cursed_other => {{\n                    __cursed_active = Some({tag_match});\n                }}\n"
                )
            } else {
                String::new()
            };
            let expected = oxford_join(
                &cf.carriers
                    .iter()
                    .map(|c| c.tag.clone())
                    .collect::<Vec<_>>(),
            );
            out.push_str(&format!(
                "    #[napi(setter, js_name = \"{js}\")]\n    pub fn {setter}(&self, {arg}: {ty}) -> napi::Result<()> {{\n        let mut __cursed_new = Some({arg});\n        let mut __cursed_active: Option<&'static str> = None;\n        let __cursed_proj = self.{root}.borrow().clone();\n        __cursed_proj.with_mut(&mut |__cursed_v| {{\n            match __cursed_v {{\n{write_arms}{catch_all}            }}\n        }})?;\n        if let Some(__cursed_active) = __cursed_active {{\n            return Err(napi::Error::from_reason(format!(\n                \"cannot set \\\"{js}\\\" on \\\"{enum_js}\\\": active variant is \\\"{{__cursed_active}}\\\", expected {expected}\"\n            )));\n        }}\n        Ok(())\n    }}\n",
                setter = mangle::setter_fn(js),
            ));
        }
        out
    }

    /// Getter/setter for a class-typed variant field. The getter hands back a
    /// live child wrapper rooted at a `VariantProjector` (cached per
    /// `(variant, field)` for referential stability); accessing it after the
    /// parent's variant changes throws `StaleRef`. The wrong active variant
    /// reads as `undefined` and writes throw.
    fn print_enum_class_field_getset(
        &self,
        orig: &str,
        enum_js: &str,
        brand: &str,
        variants: &[EnumVariantDef],
        cf: &CoalescedField,
        child: IrId,
    ) -> String {
        let js = &cf.js_name;
        let root = mangle::ROOT_FIELD;
        let child_mangled = self.ir.get_class_unchecked(child).mangled_name.clone();
        let child_orig = self.class_orig(child);
        let tag_match = self.enum_tag_match(orig, variants, "__cursed_v");
        let proj_tag_match = self.enum_tag_match(orig, variants, "__cursed_p");
        let child_lit = self.empty_wrapper_literal(child, "__cursed_cell");

        let mut arms = String::new();
        for c in &cf.carriers {
            let slot = mangle::variant_cache_field(&c.rust_name, &c.orig_name);
            let bind = raw_ident(&c.orig_name);
            arms.push_str(&format!(
                "            \"{tag}\" => {{\n                if let Some(__cursed_cell) = self.{slot}.as_ref().and_then(Weak::upgrade) {{\n                    return Ok(napi::bindgen_prelude::Either::A({child_lit}));\n                }}\n                let __cursed_proj: Rc<dyn __cursed::Projection<{child_orig}>> = Rc::new(__cursed::VariantProjector {{\n                    parent: self.{root}.clone(),\n                    project: |__cursed_p| match __cursed_p {{ {orig}::{rust} {{ {bind}, .. }} => Some({bind}), _ => None }},\n                    project_mut: |__cursed_p| match __cursed_p {{ {orig}::{rust} {{ {bind}, .. }} => Some({bind}), _ => None }},\n                    current_tag: |__cursed_p| {proj_tag_match},\n                    expected_tag: \"{tag}\",\n                    enum_path: \"{brand}\",\n                    field_name: \"{js}\",\n                }});\n                let __cursed_cell = Rc::new(RefCell::new(__cursed_proj));\n                self.{slot} = Some(Rc::downgrade(&__cursed_cell));\n                Ok(napi::bindgen_prelude::Either::A({child_lit}))\n            }}\n",
                tag = c.tag,
                rust = c.rust_name,
            ));
        }

        let mut out = String::new();
        out.push_str(&format!(
            "    #[napi(getter, js_name = \"{js}\")]\n    pub fn {getter}(&mut self) -> napi::Result<napi::bindgen_prelude::Either<{child_mangled}, ()>> {{\n        let mut __cursed_active: &'static str = \"\";\n        self.{root}.borrow().with(&mut |__cursed_v| {{ __cursed_active = {tag_match}; }})?;\n        match __cursed_active {{\n{arms}            _ => Ok(napi::bindgen_prelude::Either::B(())),\n        }}\n    }}\n",
            getter = mangle::getter_fn(js),
        ));

        if cf.has_setter {
            let arg = mangle::arg_name(js);
            let child_coerce = format!("__cursed_coerce_{child_mangled}");
            let tags = cf
                .carriers
                .iter()
                .map(|c| format!("\"{}\"", c.tag))
                .collect::<Vec<_>>()
                .join(" | ");
            let mut assign_arms = String::new();
            for c in &cf.carriers {
                let bind = raw_ident(&c.orig_name);
                assign_arms.push_str(&format!(
                    "                        {orig}::{rust} {{ {bind}, .. }} => *{bind} = __cursed_new.take().unwrap(),\n",
                    rust = c.rust_name,
                ));
            }
            let expected = oxford_join(
                &cf.carriers
                    .iter()
                    .map(|c| c.tag.clone())
                    .collect::<Vec<_>>(),
            );
            // Snapshot any live child projection into a standalone cell before
            // overwriting the parent's field, so handles taken before the
            // reassignment keep observing the old value (detach-on-reassign).
            let mut detach_arms = String::new();
            for c in &cf.carriers {
                let slot = mangle::variant_cache_field(&c.rust_name, &c.orig_name);
                detach_arms.push_str(&format!(
                    "                if let Some(__cursed_cell) = self.{slot}.as_ref().and_then(Weak::upgrade) {{\n                    let __cursed_proj = __cursed_cell.borrow().clone();\n                    let mut __cursed_snap: Option<{child_orig}> = None;\n                    __cursed_proj.with(&mut |__cursed_p| __cursed_snap = Some(__cursed_p.clone()))?;\n                    *__cursed_cell.borrow_mut() = Rc::new(RefCell::new(__cursed_snap.unwrap())) as Rc<dyn __cursed::Projection<{child_orig}>>;\n                    self.{slot} = None;\n                }}\n",
                ));
            }
            out.push_str(&format!(
                "    #[napi(setter, js_name = \"{js}\")]\n    pub fn {setter}(&mut self, {arg}: napi::bindgen_prelude::Unknown<'_>) -> napi::Result<()> {{\n        let mut __cursed_active: &'static str = \"\";\n        self.{root}.borrow().with(&mut |__cursed_v| {{ __cursed_active = {tag_match}; }})?;\n        match __cursed_active {{\n            {tags} => {{\n                let __cursed_val: {child_orig} = {child_coerce}({arg})?.into_owned()?;\n                let mut __cursed_new = Some(__cursed_val);\n{detach_arms}                self.{root}.borrow().with_mut(&mut |__cursed_v| {{\n                    match __cursed_v {{\n{assign_arms}                        _ => {{}}\n                    }}\n                }})?;\n                Ok(())\n            }}\n            __cursed_other => Err(napi::Error::from_reason(format!(\n                \"cannot set \\\"{js}\\\" on \\\"{enum_js}\\\": active variant is \\\"{{__cursed_other}}\\\", expected {expected}\"\n            ))),\n        }}\n    }}\n",
                setter = mangle::setter_fn(js),
            ));
        }
        out
    }

    /// Per-enum runtime coercer: class-instance fast path, then (when object
    /// coercible) a tagged-union object path dispatching on the discriminator.
    fn print_enum_coercer(&self, cid: IrId) -> String {
        let def = self.ir.get_class_unchecked(cid).clone();
        let orig = rust_path(&def.path, &self.ir.crate_name);
        let mangled = &def.mangled_name;
        let coerce = format!("__cursed_coerce_{mangled}");
        let ClassShape::Enum {
            variants,
            discriminator,
        } = &def.shape
        else {
            unreachable!("print_enum_coercer on non-enum")
        };

        let mut out = String::new();
        out.push_str(&format!(
            "#[allow(dead_code)]\nfn {coerce}<'a>(val: napi::bindgen_prelude::Unknown<'a>) -> napi::Result<__cursed::Coerced<{orig}>> {{\n"
        ));
        out.push_str(&format!(
            "    let __cursed_env_raw = napi::JsValue::value(&val).env;\n    let __cursed_raw = napi::JsValue::raw(&val);\n    if let Ok(__cursed_inst) = unsafe {{ napi::bindgen_prelude::ClassInstance::<{mangled}>::from_napi_value(__cursed_env_raw, __cursed_raw) }} {{\n        return Ok(__cursed::Coerced::Projected(__cursed_inst.{root}.borrow().clone()));\n    }}\n",
            root = mangle::ROOT_FIELD,
        ));

        if def.struct_expr.is_some() {
            out.push_str("    let __cursed_obj = val.coerce_to_object()?;\n");
            out.push_str(&format!(
                "    let __cursed_tag: String = __cursed_obj.get_named_property(\"{discriminator}\")?;\n"
            ));
            out.push_str("    match __cursed_tag.as_str() {\n");
            for v in variants {
                if v.is_unit {
                    out.push_str(&format!(
                        "        \"{tag}\" => Ok(__cursed::Coerced::Owned({orig}::{rust})),\n",
                        tag = v.tag,
                        rust = v.rust_name,
                    ));
                    continue;
                }
                let mut reads = String::new();
                let mut inits = Vec::new();
                for f in &v.fields {
                    let bind = raw_ident(&f.orig_name);
                    match &f.ty {
                        Type::Primitive(_) => {
                            let fty = self.type_to_rs(&f.ty);
                            reads.push_str(&format!(
                                "            let {bind}: {fty} = __cursed_obj.get_named_property(\"{js}\")?;\n",
                                js = f.js_name,
                            ));
                        }
                        Type::Class(child) => {
                            let child_coerce = format!(
                                "__cursed_coerce_{}",
                                self.ir.get_class_unchecked(*child).mangled_name
                            );
                            let child_orig = self.class_orig(*child);
                            reads.push_str(&format!(
                                "            let {bind}: {child_orig} = {child_coerce}(__cursed_obj.get_named_property::<napi::bindgen_prelude::Unknown>(\"{js}\")?)?.into_owned()?;\n",
                                js = f.js_name,
                            ));
                        }
                    }
                    // Struct-literal field shorthand: `r#type` names field
                    // `type` from local `r#type`.
                    inits.push(bind);
                }
                out.push_str(&format!(
                    "        \"{tag}\" => {{\n{reads}            Ok(__cursed::Coerced::Owned({orig}::{rust} {{ {inits} }}))\n        }}\n",
                    tag = v.tag,
                    rust = v.rust_name,
                    inits = inits.join(", "),
                ));
            }
            let taglist = variants
                .iter()
                .map(|v| format!("\\\"{}\\\"", v.tag))
                .collect::<Vec<_>>()
                .join(" | ");
            out.push_str(&format!(
                "        __cursed_other => Err(napi::Error::from_reason(format!(\n            \"expected \\\"{discriminator}\\\" to be one of {taglist}; got {{__cursed_other:?}}\"\n        ))),\n"
            ));
            out.push_str("    }\n");
        } else {
            out.push_str(&format!(
                "    Err(napi::Error::from_reason(\n        \"expected a live \\\"{js}\\\" instance; \\\"{js}\\\" cannot be built from an object literal (it is not Clone)\".to_string()\n    ))\n",
                js = def.js_name,
            ));
        }
        out.push_str("}\n");
        out
    }

    fn print_ctor(&self, def: &ClassDef, ctor: &CtorDef, orig: &str) -> String {
        let params = &ctor.params;

        // Each struct-typed arg is coerced into __cursed::Coerced<T> and then
        // taken by value (into_owned) when handed to the user fn.
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
                    format!("{}.into_owned()?", co_local(&p.name))
                } else {
                    mangle::arg_name(&p.name)
                }
            })
            .collect();
        let built = format!("{orig}::new({})", args.join(", "));

        let root_init = self.wrap_into_base_projection(orig, &built);
        let cache_inits = self.cache_init_lines(def);

        format!(
            "    #[napi(constructor)]\n    pub fn new({}) -> napi::Result<Self> {{\n{body}        Ok(Self {{\n            {}: {root_init},\n{cache_inits}        }})\n    }}\n",
            self.render_ctor_params(params),
            mangle::ROOT_FIELD,
        )
    }

    fn print_of(&self, def: &ClassDef, _se: &StructExpr, orig: &str) -> String {
        let coerce = format!("__cursed_coerce_{}", def.mangled_name);
        let root_init = self.wrap_into_base_projection(orig, "__cursed_owned");
        let js_name = if def.of_escaped { "of$" } else { "of" };
        // Rust ident stays `of` either way; `of$` is JS-only.
        let rust_ident = "of";
        let cache_inits = self.cache_init_lines(def);

        format!(
            "    #[napi(factory, js_name = \"{js_name}\")]\n    pub fn {rust_ident}(init: napi::bindgen_prelude::Unknown<'_>) -> napi::Result<Self> {{\n        let __cursed_owned: {orig} = {coerce}(init)?.into_owned()?;\n        Ok(Self {{\n            {}: {root_init},\n{cache_inits}        }})\n    }}\n",
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
                "    #[napi(getter, js_name = \"{js}\")]\n    pub fn {getter}(&self) -> napi::Result<{ty}> {{\n        let __cursed_proj = self.{root}.borrow().clone();\n        let mut __cursed_out: Option<{ty}> = None;\n        __cursed_proj.with(&mut |__cursed_p| __cursed_out = Some(__cursed_p.{orig}{clone}))?;\n        Ok(__cursed_out.unwrap())\n    }}\n",
                getter = mangle::getter_fn(orig),
                root = mangle::ROOT_FIELD,
            ));
        }
        if field.has_setter {
            // The assignment runs inside an `FnMut`, which can't move a
            // captured non-`Copy` value (e.g. `String`) out; `Option::take`
            // lifts it without re-cloning and stays uniform for `Copy` types.
            out.push_str(&format!(
                "    #[napi(setter, js_name = \"{js}\")]\n    pub fn {setter}(&self, {arg}: {ty}) -> napi::Result<()> {{\n        let __cursed_proj = self.{root}.borrow().clone();\n        let mut __cursed_arg = Some({arg});\n        __cursed_proj.with_mut(&mut |__cursed_p| __cursed_p.{orig} = __cursed_arg.take().unwrap())?;\n        Ok(())\n    }}\n",
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
                "    #[napi(getter, js_name = \"{js}\")]\n    pub fn {getter}(&mut self) -> {child_mangled} {{\n        if let Some(__cursed_cell) = self.{cache}.as_ref().and_then(Weak::upgrade) {{\n            return {child_lit};\n        }}\n        let __cursed_proj: Rc<dyn __cursed::Projection<{child_orig}>> = Rc::new(__cursed::Projector {{\n            parent: self.{root}.clone(),\n            project: |__cursed_p| &__cursed_p.{orig},\n            project_mut: |__cursed_p| &mut __cursed_p.{orig},\n        }});\n        let __cursed_cell = Rc::new(RefCell::new(__cursed_proj));\n        self.{cache} = Some(Rc::downgrade(&__cursed_cell));\n        {child_lit}\n    }}\n",
                getter = mangle::getter_fn(orig),
            ));
        }
        if field.has_setter {
            let arg = mangle::arg_name(orig);
            let (param_ty, acquire) = if self.ir.get_class_unchecked(child).struct_expr.is_some() {
                let child_coerce = format!("__cursed_coerce_{child_mangled}");
                (
                        "napi::bindgen_prelude::Unknown<'_>".to_string(),
                        format!("let mut __cursed_new_val: Option<{child_orig}> = Some({child_coerce}({arg})?.into_owned()?);"),
                    )
            } else {
                (
                        format!("&{child_mangled}"),
                        format!("let mut __cursed_new_val: Option<{child_orig}> = None;\n        {arg}.{root}.borrow().with(&mut |__cursed_p| __cursed_new_val = Some(__cursed_p.clone()))?;"),
                    )
            };
            out.push_str(&format!(
                "    #[napi(setter, js_name = \"{js}\")]\n    pub fn {setter}(&mut self, {arg}: {param_ty}) -> napi::Result<()> {{\n        {acquire}\n        if let Some(__cursed_cell) = self.{cache}.as_ref().and_then(Weak::upgrade) {{\n            let __cursed_proj = __cursed_cell.borrow().clone();\n            let mut __cursed_snap: Option<{child_orig}> = None;\n            __cursed_proj.with(&mut |__cursed_p| __cursed_snap = Some(__cursed_p.clone()))?;\n            *__cursed_cell.borrow_mut() = Rc::new(RefCell::new(__cursed_snap.unwrap())) as Rc<dyn __cursed::Projection<{child_orig}>>;\n            self.{cache} = None;\n        }}\n        let __cursed_parent = self.{root}.borrow().clone();\n        __cursed_parent.with_mut(&mut |__cursed_p| {{\n            __cursed_p.{orig} = __cursed_new_val.take().unwrap();\n        }})?;\n        Ok(())\n    }}\n",
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

        // Outer-scope statements: coerce each struct param into a
        // `__cursed::Coerced<T>`, stash owned `String`s.
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
                (Type::Primitive(prim), ParamMode::ByMut) if !matches!(prim, Primitive::Str) => {
                    pre.push_str(&format!(
                        "    let mut {mref} = {name};\n",
                        mref = mref_local(&p.name),
                        name = p.name,
                    ));
                }
                _ => {}
            }
        }

        // For by-value struct params, hoist `into_owned()` outside the
        // closure so the call site is a simple `.clone()` (the owned value
        // is cloned at the call so the local can be reused if the closure
        // somehow re-runs; in practice it runs exactly once).
        for p in &fn_def.params {
            if matches!(p.ty, Type::Class(_)) && matches!(p.mode, ParamMode::ByValue) {
                pre.push_str(&format!(
                    "    let {val} = {co}.into_owned()?;\n",
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
            let inner = if is_unit {
                format!("{call};")
            } else {
                format!("__cursed_ret = Some({call});")
            };
            let mut wraps: Vec<(String, &str, String)> = Vec::new();
            for p in &fn_def.params {
                let method = match (&p.ty, &p.mode) {
                    (Type::Class(_), ParamMode::ByRef) => "with",
                    (Type::Class(_), ParamMode::ByMut) => "with_mut",
                    _ => continue,
                };
                wraps.push((co_local(&p.name), method, format!("__cursed_{}", p.name)));
            }
            body.push_str(&self.build_with_chain(&wraps, &inner));
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
        out.push_str(&format!("#[napi(js_name = \"{glue}\")]\n"));
        let params_sig = self.render_fn_params(&fn_def.params);
        if is_unit {
            out.push_str(&format!(
                "pub fn {glue}({params_sig}) -> napi::Result<()> {{\n{body}}}\n"
            ));
        } else {
            let ret_ty = self.type_to_rs(&fn_def.return_type);
            out.push_str(&format!(
                "pub fn {glue}({params_sig}) -> napi::Result<{ret_ty}> {{\n{body}}}\n"
            ));
        }
        out
    }

    fn print_method(&self, class_id: IrId, fn_def: &FnDef, class_orig: &str) -> String {
        let receiver = fn_def.receiver;
        let js_name = &fn_def.js_name;
        let method_name = fn_def.path.name().unwrap_or("anon");
        let method_ident = mangle::method_fn(method_name);
        // UFCS: rustdoc loses the impl context in `path`, so it would render
        // as just the method name. Reach back to the class's canonical path
        // so the user fn resolves unambiguously across modules and impls.
        let user_path = format!("{class_orig}::{}", raw_ident(method_name));
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
                    "        let __cursed_self_val: {class_orig} = {{\n            let __cursed_proj = self.{root}.borrow().clone();\n            let mut __cursed_out: Option<{class_orig}> = None;\n            __cursed_proj.with(&mut |__cursed_p| __cursed_out = Some(__cursed_p.clone()))?;\n            __cursed_out.unwrap()\n        }};\n",
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
                (Type::Primitive(prim), ParamMode::ByMut) if !matches!(prim, Primitive::Str) => {
                    pre.push_str(&format!(
                        "        let mut {mref} = {name};\n",
                        mref = mref_local(&p.name),
                        name = p.name,
                    ));
                }
                _ => {}
            }
        }
        for p in &fn_def.params {
            if matches!(p.ty, Type::Class(_)) && matches!(p.mode, ParamMode::ByValue) {
                pre.push_str(&format!(
                    "        let {val} = {co}.into_owned()?;\n",
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
            let inner = if is_unit {
                format!("{call};")
            } else {
                format!("__cursed_ret = Some({call});")
            };
            let mut wraps: Vec<(String, &str, String)> = Vec::new();
            match receiver {
                Some(Receiver::ByRef) => wraps.push((
                    "__cursed_proj_self".to_string(),
                    "with",
                    "__cursed_self".to_string(),
                )),
                Some(Receiver::ByMut) => wraps.push((
                    "__cursed_proj_self".to_string(),
                    "with_mut",
                    "__cursed_self".to_string(),
                )),
                _ => {}
            }
            for p in &fn_def.params {
                let method = match (&p.ty, &p.mode) {
                    (Type::Class(_), ParamMode::ByRef) => "with",
                    (Type::Class(_), ParamMode::ByMut) => "with_mut",
                    _ => continue,
                };
                wraps.push((co_local(&p.name), method, format!("__cursed_{}", p.name)));
            }
            body.push_str(&self.build_with_chain(&wraps, &inner));
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

        let returns_self = matches!(
            &fn_def.return_type,
            Type::Class(cid) if *cid == class_id,
        );
        let attr = if receiver.is_none() && returns_self {
            format!("    #[napi(factory, js_name = \"{js_name}\")]\n")
        } else {
            format!("    #[napi(js_name = \"{js_name}\")]\n")
        };

        let mut out = String::new();
        if let Some(docs) = &fn_def.docs {
            out.push_str(&rust_doc(docs, "    "));
        }
        out.push_str(&attr);
        if is_unit {
            out.push_str(&format!(
                "    pub fn {method_ident}({head}) -> napi::Result<()> {{\n{body}    }}\n"
            ));
        } else {
            let ret_ty = self.type_to_rs(&fn_def.return_type);
            out.push_str(&format!(
                "    pub fn {method_ident}({head}) -> napi::Result<{ret_ty}> {{\n{body}    }}\n"
            ));
        }
        out
    }

    /// Build the nested `with`/`with_mut` closure chain that runs `inner`
    /// with the receiver and ref-class params bound. Each `with` closure
    /// returns `()`, so only the outermost call can use `?` directly; inner
    /// levels capture their `Result<(), StaleRef>` into `__cursed_stale_<k>`
    /// vars that are re-checked (`?`) after the chain unwinds. `wraps` is
    /// ordered outermost-first as `(co_expr, method, bind_ident)`.
    fn build_with_chain(&self, wraps: &[(String, &str, String)], inner: &str) -> String {
        if wraps.is_empty() {
            return format!("    {inner}\n");
        }
        let mut decls = String::new();
        for k in 1..wraps.len() {
            decls.push_str(&format!(
                "    let mut __cursed_stale_{k}: Result<(), __cursed::StaleRef> = Ok(());\n"
            ));
        }
        let mut body = inner.to_string();
        let mut post = String::new();
        for (k, (co, method, bind)) in wraps.iter().enumerate().rev() {
            if k == 0 {
                body = format!("{co}.{method}(&mut |{bind}| {{\n{body}\n}})?;");
            } else {
                body = format!("__cursed_stale_{k} = {co}.{method}(&mut |{bind}| {{\n{body}\n}});");
                post = format!("    __cursed_stale_{k}?;\n{post}");
            }
        }
        format!("{decls}    {body}\n{post}")
    }

    /// `Rc::new(RefCell::new(Rc::new(RefCell::new(<value>)) as Rc<dyn ...>))`.
    fn wrap_into_base_projection(&self, orig_ty: &str, value_expr: &str) -> String {
        format!(
            "Rc::new(RefCell::new(Rc::new(RefCell::new({value_expr})) as Rc<dyn __cursed::Projection<{orig_ty}>>))"
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
        for (slot, _) in self.class_cache_slots(def) {
            s.push_str(&format!(", {slot}: None"));
        }
        s.push_str(" }");
        s
    }

    /// `<slot>: None,` initializer lines for every cache slot of `def`.
    fn cache_init_lines(&self, def: &ClassDef) -> String {
        let mut s = String::new();
        for (slot, _) in self.class_cache_slots(def) {
            s.push_str(&format!("            {slot}: None,\n"));
        }
        s
    }

    /// Every class-typed cache slot of `def` as `(slot_name, child_orig)`.
    /// Struct wrappers cache per struct field; data-enum wrappers cache per
    /// class-typed `(variant, field)` so different per-variant child types
    /// never share storage.
    fn class_cache_slots(&self, def: &ClassDef) -> Vec<(String, String)> {
        let mut out = Vec::new();
        match &def.shape {
            ClassShape::Struct { fields } => {
                for field in fields {
                    if let Type::Class(child) = &field.ty {
                        out.push((
                            mangle::cache_field(&field.orig_name),
                            self.class_orig(*child),
                        ));
                    }
                }
            }
            ClassShape::Enum { variants, .. } => {
                for v in variants {
                    for f in &v.fields {
                        if let Type::Class(child) = &f.ty {
                            out.push((
                                mangle::variant_cache_field(&v.rust_name, &f.orig_name),
                                self.class_orig(*child),
                            ));
                        }
                    }
                }
            }
        }
        out
    }

    /// Coerce a struct-typed ctor arg into a local `__cursed_co_<name>`.
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
            (Type::Class(_), ParamMode::ByValue) => val_local(&p.name),
            (Type::Primitive(Primitive::Str), _) => format!("&{}", p.name),
            (Type::Primitive(_), ParamMode::ByMut) if p.needs_stash => {
                format!("&mut {}.take().unwrap()", owned_local(&p.name))
            }
            (Type::Primitive(_), ParamMode::ByMut) => format!("&mut {}", mref_local(&p.name)),
            (Type::Primitive(_), ParamMode::ByRef) if p.needs_stash => {
                format!("&{}.take().unwrap()", owned_local(&p.name))
            }
            (Type::Primitive(_), ParamMode::ByRef) => format!("&{}", p.name),
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

    /// Ctor params take `Unknown<'_>` for struct-typed args (coerced inside),
    /// or the native primitive otherwise.
    fn ctor_param_type_to_rs(&self, p: &FnParam) -> String {
        if matches!(p.ty, Type::Class(_)) {
            return "napi::bindgen_prelude::Unknown<'_>".to_string();
        }
        self.type_to_rs(&p.ty)
    }

    /// Fn-glue params take `Unknown<'_>` for struct-typed args, native
    /// primitive otherwise.
    fn fn_param_type_to_rs(&self, p: &FnParam) -> String {
        if matches!(p.ty, Type::Class(_)) {
            return "napi::bindgen_prelude::Unknown<'_>".to_string();
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

fn mref_local(name: &str) -> String {
    format!("__cursed_{name}_mref")
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

fn rust_doc(docs: &str, indent: &str) -> String {
    let trimmed = docs.trim();
    let mut out = String::new();
    for line in trimmed.lines() {
        out.push_str(&format!("{indent}/// {}\n", line.trim()));
    }
    out
}
