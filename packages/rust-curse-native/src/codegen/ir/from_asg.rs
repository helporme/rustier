//! Lowers an [`Asg`] into the [`Ir`].

use std::collections::{HashMap, HashSet};

use crate::codegen::asg::{
    self as asg, Asg, Item as AsgItem, Span as AsgSpan, TraitRef, WellKnownTrait,
};
use crate::codegen::mangle::mangle;
use crate::warns::{Warn, WarnLabel, Warns};

use super::{ClassDef, ClassField};
use super::{Ctor, FnDef, FnParam, Ir, IrId, Item, ModDef, Type, Use};

/// Errors that may be produced by [`Ir::from_asg`].

#[derive(Debug, thiserror::Error)]
pub enum FromAsgError {
    /// Reserved sentinel - never constructed by today's lowering.
    #[error("unreachable")]
    Unreachable,
}

impl Ir {
    /// Lower an [`Asg`] into the per-target [`Ir`].
    pub fn from_asg(warns: &mut Warns, asg: &Asg) -> Result<Ir, FromAsgError> {
        Ok(Lowerer::new(warns, asg).run())
    }
}

struct Lowerer<'a> {
    warns: &'a mut Warns,
    asg: &'a Asg,
    /// Fns that are associated (methods / ctors) -> never lowered as free fns.
    method_fns: HashSet<asg::AsgId>,
    /// Structs that survive lowering as JS classes.
    emittable: HashSet<asg::AsgId>,
    /// Maps a surviving ASG item id to its IR id.
    asg_to_ir: HashMap<asg::AsgId, IrId>,
    /// Per-struct field names whose setter is suppressed because the field's
    /// child struct doesn't implement `Clone`.
    suppressed_setters: HashMap<asg::AsgId, HashSet<String>>,
    /// Structs whose implicit ctor is suppressed because at least one
    /// struct-typed field's child isn't `Clone`.
    suppressed_implicit_ctors: HashSet<asg::AsgId>,
}

impl<'a> Lowerer<'a> {
    fn new(warns: &'a mut Warns, asg: &'a Asg) -> Self {
        Self {
            warns,
            asg,
            method_fns: HashSet::new(),
            emittable: HashSet::new(),
            asg_to_ir: HashMap::new(),
            suppressed_setters: HashMap::new(),
            suppressed_implicit_ctors: HashSet::new(),
        }
    }

    fn run(mut self) -> Ir {
        self.collect_method_fns();
        self.select_classes();
        self.assign_ids();

        let items: Vec<Item> = self
            .asg
            .items
            .iter()
            .filter_map(|item| self.lower_item(item))
            .collect();

        let uses_projections = items.iter().any(|i| matches!(i, Item::Class(_)));

        Ir {
            root: self.asg_to_ir[&self.asg.root],
            crate_name: self.asg.crate_name.clone(),
            items,
            uses_projections,
        }
    }

    fn collect_method_fns(&mut self) {
        for item in &self.asg.items {
            if let AsgItem::Impl(impl_) = item {
                for fid in &impl_.fns {
                    self.method_fns.insert(*fid);
                }
            }
        }
    }

    fn select_classes(&mut self) {
        for item in &self.asg.items {
            let AsgItem::Struct(def) = item else { continue };
            match &def.shape {
                asg::StructShape::Struct(_) => {
                    self.emittable.insert(def.id);
                }
                asg::StructShape::Tuple(_) => {
                    self.warns.push(attach_span(
                        warn_unsupported_type("tuple structs are not yet supported"),
                        &def.span,
                    ));
                }
                asg::StructShape::Unit => {
                    self.warns.push(attach_span(
                        warn_unsupported_type("unit structs are not yet supported"),
                        &def.span,
                    ));
                }
            }
        }

        // Drop structs that reference an upstream-skipped struct (tuple, unit,
        // etc.) — that's an unresolvable type. Repeat to fixpoint so chained
        // references collapse correctly.
        loop {
            let mut dropped: Option<(asg::AsgId, Warn, Option<AsgSpan>)> = None;
            for &sid in &self.emittable {
                let def = self.asg.get_struct_unchecked(sid);
                let asg::StructShape::Struct(fields) = &def.shape else {
                    continue;
                };
                for f in fields {
                    let asg::Type::Struct(child) = f.ty else {
                        continue;
                    };
                    if !self.emittable.contains(&child) {
                        let child_name = self
                            .asg
                            .get_struct_unchecked(child)
                            .path
                            .name()
                            .unwrap_or("?")
                            .to_string();
                        let warn = warn_skipped_struct_ref(child_name);
                        dropped = Some((sid, warn, def.span.clone()));
                        break;
                    }
                }
                if dropped.is_some() {
                    break;
                }
            }
            let Some((sid, warn, span)) = dropped else {
                break;
            };
            self.emittable.remove(&sid);
            self.warns.push(attach_span(warn, &span));
        }

        let emittable: Vec<asg::AsgId> = self.emittable.iter().copied().collect();
        for sid in emittable {
            let def = self.asg.get_struct_unchecked(sid);
            let asg::StructShape::Struct(fields) = &def.shape else {
                continue;
            };
            let parent_name = def.path.name().unwrap_or("?").to_string();
            let span = def.span.clone();
            for f in fields {
                let asg::Type::Struct(child) = f.ty else {
                    continue;
                };
                if self.asg_clonable(child) {
                    continue;
                }
                let child_name = self
                    .asg
                    .get_struct_unchecked(child)
                    .path
                    .name()
                    .unwrap_or("?")
                    .to_string();
                self.suppressed_setters
                    .entry(sid)
                    .or_default()
                    .insert(f.name.clone());
                self.suppressed_implicit_ctors.insert(sid);
                self.warns.push(attach_span(
                    warn_field_setter_unavailable(&parent_name, &f.name, &child_name),
                    &span,
                ));
            }
        }
    }

    fn assign_ids(&mut self) {
        let mut next = 0usize;
        for item in &self.asg.items {
            let survives = match item {
                AsgItem::Mod(_) => true,
                AsgItem::Struct(def) => self.emittable.contains(&def.id),
                AsgItem::Fn(def) => !self.method_fns.contains(&def.id) && self.fn_is_emittable(def),
                AsgItem::Impl(_) | AsgItem::Trait(_) => false,
            };
            if !survives {
                continue;
            }
            let asg_id = item_id(item);
            self.asg_to_ir.insert(asg_id, IrId(next));
            next += 1;
        }
    }

    fn fn_is_emittable(&mut self, def: &asg::FnDef) -> bool {
        let mut referenced = Vec::new();
        for p in &def.params {
            referenced.push(&p.ty);
        }
        referenced.push(&def.return_type);
        for ty in referenced {
            if let asg::Type::Struct(sid) = ty {
                if !self.emittable.contains(sid) {
                    let name = self
                        .asg
                        .get_struct_unchecked(*sid)
                        .path
                        .name()
                        .unwrap_or("?")
                        .to_string();
                    self.warns
                        .push(attach_span(warn_skipped_struct_ref(name), &def.span));
                    return false;
                }
            }
        }
        // A by-value struct param is cloned into an owned value
        // before the user fn runs, so its type must be `Clone`.
        for p in &def.params {
            if p.mode != asg::ParamMode::ByValue {
                continue;
            }
            let asg::Type::Struct(sid) = &p.ty else {
                continue;
            };
            if !self.asg_clonable(*sid) {
                let struct_name = self
                    .asg
                    .get_struct_unchecked(*sid)
                    .path
                    .name()
                    .unwrap_or("?")
                    .to_string();
                let fn_name = def.path.name().unwrap_or("?").to_string();
                self.warns.push(attach_span(
                    warn_fn_byvalue_not_clonable(&fn_name, &p.name, &struct_name),
                    &def.span,
                ));
                return false;
            }
        }
        true
    }

    fn lower_item(&self, item: &AsgItem) -> Option<Item> {
        match item {
            AsgItem::Struct(def) if self.emittable.contains(&def.id) => {
                Some(Item::Class(self.lower_class(def)))
            }
            AsgItem::Fn(def) if self.asg_to_ir.contains_key(&def.id) => {
                Some(Item::Fn(self.lower_fn(def)))
            }
            AsgItem::Mod(def) => Some(Item::Mod(self.lower_mod(def))),
            _ => None,
        }
    }

    fn lower_class(&self, def: &asg::StructDef) -> ClassDef {
        let asg::StructShape::Struct(asg_fields) = &def.shape else {
            unreachable!("only named-field structs are emittable")
        };

        let suppressed_setters = self.suppressed_setters.get(&def.id);
        let path = def.path.clone();
        let fields = asg_fields
            .iter()
            .map(|f| ClassField {
                orig_name: f.name.clone(),
                js_name: f.name.clone(),
                ty: self.lower_type(&f.ty),
                has_getter: true,
                has_setter: !suppressed_setters
                    .map(|s| s.contains(&f.name))
                    .unwrap_or(false),
                clone_on_get: !is_copy_primitive(&f.ty),
                docs: f.docs.clone(),
            })
            .collect();

        let ctor = self.resolve_ctor(def, asg_fields);
        let ctor = match ctor {
            Some(Ctor::Implicit { .. }) if self.suppressed_implicit_ctors.contains(&def.id) => None,
            other => other,
        };

        ClassDef {
            id: self.asg_to_ir[&def.id],
            orig: def.id,
            js_name: path.name().unwrap_or_default().to_string(),
            mangled_name: mangle(&path),
            path,
            fields,
            ctor,
            clonable: self.is_clonable(def),
            docs: def.docs.clone(),
        }
    }

    fn is_clonable(&self, def: &asg::StructDef) -> bool {
        self.asg_clonable(def.id)
    }

    /// True when the ASG struct `sid` carries a `Clone` impl (derived or
    /// hand-written).
    fn asg_clonable(&self, sid: asg::AsgId) -> bool {
        self.asg
            .get_struct_unchecked(sid)
            .impls
            .iter()
            .any(|impl_id| {
                matches!(
                    self.asg.get_impl_unchecked(*impl_id).trait_,
                    Some(TraitRef::WellKnown(WellKnownTrait::Clone))
                )
            })
    }

    fn resolve_ctor(&self, def: &asg::StructDef, asg_fields: &[asg::StructField]) -> Option<Ctor> {
        struct Candidate<'b> {
            key: Vec<String>,
            impl_ord: usize,
            params: &'b [asg::FnParam],
        }
        let mut candidates: Vec<Candidate> = Vec::new();

        for &impl_id in &def.impls {
            let impl_ = self.asg.get_impl_unchecked(impl_id);
            let key_path: Vec<String> = match &impl_.trait_ {
                Some(TraitRef::Local(tid)) => {
                    self.asg.get_trait_unchecked(*tid).path.segments.clone()
                }
                Some(TraitRef::WellKnown(_)) => continue,
                None => def.path.segments.clone(),
            };
            for &fid in &impl_.fns {
                let f = self.asg.get_fn_unchecked(fid);
                if f.receiver.is_some() || f.path.name() != Some("new") {
                    continue;
                }
                if f.return_type != asg::Type::Struct(def.id) {
                    continue;
                }
                let mut key = key_path.clone();
                key.push("new".to_string());
                candidates.push(Candidate {
                    key,
                    impl_ord: impl_id.0,
                    params: &f.params,
                });
            }
        }

        candidates.sort_by(|a, b| a.key.cmp(&b.key).then(a.impl_ord.cmp(&b.impl_ord)));

        Some(match candidates.first() {
            Some(c) => Ctor::Explicit {
                params: c.params.iter().map(|p| self.lower_param(p)).collect(),
            },
            None => Ctor::Implicit {
                params: asg_fields
                    .iter()
                    .map(|f| FnParam {
                        name: f.name.clone(),
                        mode: asg::ParamMode::ByValue,
                        ty: self.lower_type(&f.ty),
                        needs_stash: false,
                    })
                    .collect(),
            },
        })
    }

    fn lower_fn(&self, def: &asg::FnDef) -> FnDef {
        let path = def.path.clone();
        let mut params: Vec<FnParam> = def.params.iter().map(|p| self.lower_param(p)).collect();

        // Any struct passed by `&` / `&mut` forces the user call inside a
        // `with` / `with_mut` closure; once that scope exists, owned non-`Copy`
        // primitives (String) need stashing to escape `FnMut`.
        let needs_closure_scope = params.iter().any(|p| {
            matches!(p.ty, Type::Class(_))
                && matches!(p.mode, asg::ParamMode::ByRef | asg::ParamMode::ByMut)
        });
        if needs_closure_scope {
            for p in &mut params {
                if matches!(p.ty, Type::Primitive(asg::Primitive::String)) {
                    p.needs_stash = true;
                }
            }
        }

        FnDef {
            id: self.asg_to_ir[&def.id],
            orig: def.id,
            js_name: path.name().unwrap_or_default().to_string(),
            mangled_name: mangle(&path),
            path,
            params,
            return_type: self.lower_type(&def.return_type),
            needs_closure_scope,
            docs: def.docs.clone(),
        }
    }

    fn lower_mod(&self, def: &asg::ModDef) -> ModDef {
        let uses = def
            .uses
            .iter()
            .filter_map(|u| {
                self.asg_to_ir.get(&u.target).map(|&target| Use {
                    name: u.name.clone(),
                    target,
                })
            })
            .collect();
        ModDef {
            id: self.asg_to_ir[&def.id],
            orig: def.id,
            path: def.path.clone(),
            span: def.span.clone(),
            uses,
        }
    }

    fn lower_param(&self, p: &asg::FnParam) -> FnParam {
        FnParam {
            name: p.name.clone(),
            mode: p.mode.clone(),
            ty: self.lower_type(&p.ty),
            // Set in `lower_fn` once `needs_closure_scope` is known. Ctors
            // never have closures, so the default is the final value there.
            needs_stash: false,
        }
    }

    fn lower_type(&self, ty: &asg::Type) -> Type {
        match ty {
            asg::Type::Primitive(p) => Type::Primitive(p.clone()),
            asg::Type::Struct(sid) => Type::Class(self.asg_to_ir[sid]),
        }
    }
}

fn item_id(item: &AsgItem) -> asg::AsgId {
    match item {
        AsgItem::Struct(d) => d.id,
        AsgItem::Fn(d) => d.id,
        AsgItem::Mod(d) => d.id,
        AsgItem::Impl(d) => d.id,
        AsgItem::Trait(d) => d.id,
    }
}

fn is_copy_primitive(ty: &asg::Type) -> bool {
    matches!(
        ty,
        asg::Type::Primitive(
            asg::Primitive::Boolean
                | asg::Primitive::Char
                | asg::Primitive::i8
                | asg::Primitive::u8
                | asg::Primitive::i16
                | asg::Primitive::u16
                | asg::Primitive::i32
                | asg::Primitive::u32
                | asg::Primitive::i64
                | asg::Primitive::u64
                | asg::Primitive::isize
                | asg::Primitive::usize
                | asg::Primitive::f32
                | asg::Primitive::f64,
        )
    )
}

fn warn_unsupported_type(reason: &str) -> Warn {
    Warn::new("rust-curse::unsupported_type", reason.to_string())
}

fn warn_fn_byvalue_not_clonable(fn_name: &str, param_name: &str, struct_name: &str) -> Warn {
    Warn::new(
        "rust-curse::fn_not_available",
        format!(
            "Function \"{fn_name}\" cannot be called from JS: parameter \"{param_name}\" of type \"{struct_name}\" is taken by value, but \"{struct_name}\" must implement Clone."
        ),
    )
    .help(format!(
        "Add `#[derive(Clone)]` to \"{struct_name}\", or change the parameter to `&{struct_name}` / `&mut {struct_name}` to pass it by reference instead."
    ))
}

fn warn_field_setter_unavailable(parent: &str, field: &str, child: &str) -> Warn {
    Warn::new(
        "rust-curse::field_setter_unavailable",
        format!(
            "\"{parent}.{field}\" field of type \"{child}\" cannot be set from JS. \"{child}\" must implement Clone."
        ),
    )
    .help(format!(
        "Add `#[derive(Clone)]` to \"{child}\" to enable assigning this field from JS."
    ))
}

fn warn_skipped_struct_ref(struct_name: String) -> Warn {
    Warn::new(
        "rust-curse::skipped_struct_ref",
        format!("references a skipped struct \"{struct_name}\""),
    )
    .help("fix the warning emitted for this struct to expose it")
}

fn attach_span(warn: Warn, span: &Option<AsgSpan>) -> Warn {
    match span {
        Some(span) => warn
            .label(WarnLabel::new(None, span.begin, span.end))
            .source_code(&span.filename),
        None => warn,
    }
}
