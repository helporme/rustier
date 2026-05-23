//! Creates rust-curse ASG from rustdoc.

use std::collections::HashMap;
use std::collections::HashSet;

use rustdoc_types::{
    Crate, GenericParamDefKind, Id as RId, Item as RItem, ItemEnum, Path as RPath, Span as RSpan,
    StructKind, Type as RType, Use as RUse, Visibility,
};

use super::{
    Asg, AsgId, FnDef, FnParam, ImplDef, Item, ItemPath, ModDef, ParamMode, Primitive, Receiver,
    Span, StructDef, StructField, StructShape, TraitDef, TraitRef, Type, Use, WellKnownTrait,
};
use crate::warns::Warn;
use crate::warns::WarnLabel;
use crate::warns::Warns;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("item {0:?} not found in crate index")]
    MissingItem(RId),
}

impl Asg {
    /// Build the rust-curse ASG from a rustdoc `Crate`. Unsupported items are
    /// soft-skipped and recorded in `warns`. The build succeeds as long as no
    /// `MissingItem` is encountered.
    pub fn from_rustdoc(crate_: &Crate, warns: &mut Warns) -> Result<Asg, BuildError> {
        let mut b = Builder {
            crate_,
            warns,
            asg: Asg::default(),
            struct_intern: HashMap::new(),
            fn_intern: HashMap::new(),
            mod_intern: HashMap::new(),
            trait_intern: HashMap::new(),
            self_stack: Vec::new(),
        };

        let root = b
            .build_module(crate_.root)
            .map_err(|err| {
                if let ProbeError::MissingItem(id) = err {
                    BuildError::MissingItem(id)
                } else {
                    panic!("unexpected probe error: {:?}", err);
                }
            })?
            .expect("root module has no id reservation");

        b.asg.root = root;
        b.asg.crate_name = crate_
            .index
            .get(&crate_.root)
            .and_then(|it| it.name.clone())
            .unwrap_or_default();
        Ok(b.asg)
    }
}

struct Builder<'a> {
    crate_: &'a Crate,
    warns: &'a mut Warns,
    asg: Asg,
    /// `None` means the struct was probed and skipped.
    struct_intern: HashMap<RId, Option<AsgId>>,
    /// `None` means the fn was probed and skipped.
    fn_intern: HashMap<RId, Option<AsgId>>,
    mod_intern: HashMap<RId, AsgId>,
    // `None` means the trait was probed and skipped (unsupported).
    trait_intern: HashMap<RId, Option<AsgId>>,
    self_stack: Vec<RId>,
}

impl<'a> Builder<'a> {
    fn build_module(&mut self, mod_id: RId) -> Result<Option<AsgId>, ProbeError> {
        if let Some(&mid) = self.mod_intern.get(&mod_id) {
            return Ok(Some(mid));
        }
        let item = self.get_item(mod_id)?;
        let module = match &item.inner {
            ItemEnum::Module(m) => m,
            _ => return Ok(None),
        };

        let span = item.span.as_ref().map(convert_span);
        let mid = self.reserve_mod(path_of(item, mod_id, self.crate_), span);
        self.mod_intern.insert(mod_id, mid);
        let mut uses = Vec::new();

        for child_id in &module.items {
            let Some(child) = self.crate_.index.get(child_id) else {
                continue;
            };
            if !is_public(child) {
                continue;
            }
            let child_name = child.name.clone().unwrap_or_default();
            match &child.inner {
                ItemEnum::Module(_) => {
                    if let Some(child_mid) = self.build_module(*child_id)? {
                        uses.push(Use {
                            name: child_name,
                            target: child_mid,
                        });
                    }
                }
                ItemEnum::Struct(_) => {
                    if let Some(sid) = self.intern_struct(*child_id)? {
                        uses.push(Use {
                            name: child_name,
                            target: sid,
                        });
                    }
                }
                ItemEnum::Enum(_) => {
                    self.warns
                        .push(warn_unsupported_enum().attach_span(&child.span));
                }
                ItemEnum::Function(_) => {
                    if let Some(fid) = self.intern_fn(*child_id)? {
                        uses.push(Use {
                            name: child_name,
                            target: fid,
                        });
                    }
                }
                ItemEnum::Use(use_) => {
                    let mut visited = HashSet::new();
                    self.add_use(use_, &mut uses, &mut visited, child)?;
                }
                _ => {}
            }
        }

        if uses.is_empty() && mod_id != self.crate_.root {
            self.asg.items.pop();
            self.mod_intern.remove(&mod_id);
            return Ok(None);
        }

        if let Item::Mod(m) = &mut self.asg.items[mid.0] {
            m.uses = uses;
        }
        Ok(Some(mid))
    }

    fn add_use(
        &mut self,
        use_: &RUse,
        uses: &mut Vec<Use>,
        visited: &mut HashSet<RId>,
        use_item: &RItem,
    ) -> Result<(), ProbeError> {
        let Some(tid) = use_.id else { return Ok(()) };
        let Some(target) = self.crate_.index.get(&tid) else {
            if let Some((crate_name, path)) = self.external_import(tid) {
                self.warns
                    .push(warn_unsupported_import(crate_name, path).attach_span(&use_item.span));
            }
            return Ok(());
        };

        if use_.is_glob {
            if !visited.insert(tid) {
                return Ok(());
            }
            let ItemEnum::Module(source_mod) = &target.inner else {
                return Ok(());
            };
            let child_ids: Vec<RId> = source_mod.items.clone();
            for child_id in child_ids {
                let Some(child) = self.crate_.index.get(&child_id) else {
                    continue;
                };
                if !is_public(child) {
                    continue;
                }
                let child_name = child.name.clone().unwrap_or_default();
                match &child.inner {
                    ItemEnum::Struct(_) => {
                        if let Some(sid) = self.intern_struct(child_id)? {
                            uses.push(Use {
                                name: child_name,
                                target: sid,
                            });
                        }
                    }
                    ItemEnum::Enum(_) => {
                        self.warns
                            .push(warn_unsupported_enum().attach_span(&child.span));
                    }
                    ItemEnum::Function(_) => {
                        if let Some(fid) = self.intern_fn(child_id)? {
                            uses.push(Use {
                                name: child_name,
                                target: fid,
                            });
                        }
                    }
                    ItemEnum::Module(_) => {
                        if let Some(mid) = self.build_module(child_id)? {
                            uses.push(Use {
                                name: child_name,
                                target: mid,
                            });
                        }
                    }
                    ItemEnum::Use(inner_use) => {
                        self.add_use(inner_use, uses, visited, child)?;
                    }
                    _ => {}
                }
            }
            return Ok(());
        }

        let name = use_.name.clone();
        match &target.inner {
            ItemEnum::Struct(_) if is_public(target) => {
                if let Some(sid) = self.intern_struct(tid)? {
                    uses.push(Use { name, target: sid });
                }
            }
            ItemEnum::Enum(_) if is_public(target) => {
                self.warns
                    .push(warn_unsupported_enum().attach_span(&target.span));
            }
            ItemEnum::Function(_) if is_public(target) => {
                if let Some(fid) = self.intern_fn(tid)? {
                    uses.push(Use { name, target: fid });
                }
            }
            ItemEnum::Module(_) => {
                if let Some(target_mid) = self.build_module(tid)? {
                    uses.push(Use {
                        name,
                        target: target_mid,
                    });
                }
            }
            ItemEnum::Use(inner_use) => {
                self.add_use(inner_use, uses, visited, target)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn reserve_mod(&mut self, path: ItemPath, span: Option<Span>) -> AsgId {
        let id = AsgId(self.asg.items.len());
        self.asg.items.push(Item::Mod(ModDef {
            id,
            path,
            span,
            uses: Vec::new(),
        }));
        id
    }

    fn intern_struct(&mut self, id: RId) -> Result<Option<AsgId>, ProbeError> {
        if let Some(slot) = self.struct_intern.get(&id) {
            return Ok(*slot);
        }
        let item = self.get_item(id)?;
        let struct_ = match &item.inner {
            ItemEnum::Struct(s) => s,
            _ => {
                return Err(ProbeError::UnsupportedType {
                    reason: "expected struct".into(),
                });
            }
        };

        let span = item.span.as_ref().map(convert_span);
        let path = path_of(item, id, self.crate_);
        let impl_rids = struct_.impls.clone();
        let has_private_fields = match &struct_.kind {
            StructKind::Plain {
                has_stripped_fields,
                ..
            } => *has_stripped_fields,
            _ => false,
        };

        // Dry-probe: resolve the shape without committing the slot. If any
        // field type fails, emit a warning and mark the struct skipped.
        let shape = match self.build_struct_shape(&struct_.kind) {
            Ok(s) => s,
            Err(ProbeError::MissingItem(rid)) => return Err(ProbeError::MissingItem(rid)),
            Err(ProbeError::Suppressed) => {
                // Field-level warning already emitted at precise span.
                self.struct_intern.insert(id, None);
                return Ok(None);
            }
            Err(ProbeError::SkippedStructRef { struct_name }) => {
                self.warns
                    .push(warn_skipped_struct_ref(struct_name).attach_span(&item.span));
                self.struct_intern.insert(id, None);
                return Ok(None);
            }
            Err(ProbeError::UnsupportedType { reason, .. })
            | Err(ProbeError::UnsupportedImport { path: reason, .. }) => {
                self.warns
                    .push(warn_unsupported_type(reason).attach_span(&item.span));
                self.struct_intern.insert(id, None);
                return Ok(None);
            }
        };

        let sid = AsgId(self.asg.items.len());
        self.struct_intern.insert(id, Some(sid));
        self.asg.items.push(Item::Struct(StructDef {
            id: sid,
            path,
            span,
            shape,
            impls: Vec::new(),
            docs: item.docs.clone(),
            has_private_fields,
        }));

        let impls = self.build_impls(id, sid, &impl_rids)?;

        if let Item::Struct(def) = &mut self.asg.items[sid.0] {
            def.impls = impls;
        }
        Ok(Some(sid))
    }

    fn build_impls(
        &mut self,
        struct_rid: RId,
        struct_sid: AsgId,
        impl_rids: &[RId],
    ) -> Result<Vec<AsgId>, ProbeError> {
        let mut out = Vec::new();
        for &impl_rid in impl_rids {
            let Some(impl_item) = self.crate_.index.get(&impl_rid) else {
                continue;
            };
            let ItemEnum::Impl(impl_) = &impl_item.inner else {
                continue;
            };
            if impl_.is_negative {
                continue;
            }

            let trait_path = impl_.trait_.clone();
            let assoc_ids = impl_.items.clone();

            let trait_ = match trait_path {
                None => None,
                Some(tp) => {
                    if let Some(wk) = recognize_well_known(self.crate_, &tp) {
                        Some(TraitRef::WellKnown(wk))
                    } else if self
                        .crate_
                        .index
                        .get(&tp.id)
                        .map(|i| matches!(i.inner, ItemEnum::Trait(_)))
                        .unwrap_or(false)
                    {
                        match self.intern_trait(tp.id)? {
                            Some(tid) => Some(TraitRef::Local(tid)),
                            // Unsupported trait -> skip the whole impl.
                            None => continue,
                        }
                    } else {
                        // External, non-well-known trait -> skip the impl.
                        continue;
                    }
                }
            };

            self.self_stack.push(struct_rid);
            let mut fns = Vec::new();
            for assoc_id in assoc_ids {
                let Some(assoc) = self.crate_.index.get(&assoc_id) else {
                    continue;
                };
                if !matches!(assoc.inner, ItemEnum::Function(_)) {
                    continue;
                }
                if let Some(fid) = self.intern_fn(assoc_id)? {
                    fns.push(fid);
                }
            }
            self.self_stack.pop();

            let impl_id = AsgId(self.asg.items.len());
            self.asg.items.push(Item::Impl(ImplDef {
                id: impl_id,
                for_: struct_sid,
                trait_: trait_.clone(),
                fns,
            }));

            if let Some(TraitRef::Local(tid)) = &trait_ {
                if let Item::Trait(t) = &mut self.asg.items[tid.0] {
                    t.impls.push(impl_id);
                }
            }
            out.push(impl_id);
        }
        Ok(out)
    }

    /// Intern a local trait as a [`TraitDef`]. Traits with type/const generic
    /// params, associated consts, or associated types are unsupported and
    /// return `None`. Required fns are not interned.
    fn intern_trait(&mut self, id: RId) -> Result<Option<AsgId>, ProbeError> {
        if let Some(slot) = self.trait_intern.get(&id) {
            return Ok(*slot);
        }
        let item = self.get_item(id)?;
        let ItemEnum::Trait(tr) = &item.inner else {
            self.trait_intern.insert(id, None);
            return Ok(None);
        };

        let has_ty_const_generics = tr.generics.params.iter().any(|p| {
            matches!(
                p.kind,
                GenericParamDefKind::Type { .. } | GenericParamDefKind::Const { .. }
            )
        });
        let has_assoc = tr.items.iter().any(|it| {
            self.crate_
                .index
                .get(it)
                .map(|i| {
                    matches!(
                        i.inner,
                        ItemEnum::AssocConst { .. } | ItemEnum::AssocType { .. }
                    )
                })
                .unwrap_or(false)
        });
        if has_ty_const_generics || has_assoc {
            self.trait_intern.insert(id, None);
            return Ok(None);
        }

        let span = item.span.as_ref().map(convert_span);
        let path = path_of(item, id, self.crate_);
        let tid = AsgId(self.asg.items.len());
        self.trait_intern.insert(id, Some(tid));
        self.asg.items.push(Item::Trait(TraitDef {
            id: tid,
            path,
            span,
            fns: Vec::new(),
            impls: Vec::new(),
            docs: item.docs.clone(),
        }));
        Ok(Some(tid))
    }

    fn build_struct_shape(&mut self, kind: &StructKind) -> Result<StructShape, ProbeError> {
        match kind {
            StructKind::Unit => Ok(StructShape::Unit),
            StructKind::Tuple(fields) => {
                let mut out = Vec::new();
                for opt_id in fields {
                    let Some(fid) = opt_id else { continue };
                    let field_item = self.get_item(*fid)?;
                    let ItemEnum::StructField(ty) = &field_item.inner else {
                        continue;
                    };
                    out.push(self.resolve_field_type(ty, field_item)?);
                }
                Ok(StructShape::Tuple(out))
            }
            StructKind::Plain { fields, .. } => {
                let mut out = Vec::new();
                for fid in fields {
                    let field_item = self.get_item(*fid)?;
                    let ItemEnum::StructField(ty) = &field_item.inner else {
                        continue;
                    };
                    out.push(StructField {
                        name: field_item.name.clone().unwrap_or_default(),
                        ty: self.resolve_field_type(ty, field_item)?,
                        docs: field_item.docs.clone(),
                    });
                }
                Ok(StructShape::Struct(out))
            }
        }
    }

    /// Resolve a struct field's type. On failure, emit the warning anchored
    /// at the field's own span (precise underline) and return `Suppressed`
    /// so the enclosing struct skips silently.
    fn resolve_field_type(&mut self, ty: &RType, field_item: &RItem) -> Result<Type, ProbeError> {
        match self.resolve_type(ty) {
            Ok(t) => Ok(t),
            Err(ProbeError::MissingItem(rid)) => Err(ProbeError::MissingItem(rid)),
            Err(ProbeError::Suppressed) => Err(ProbeError::Suppressed),
            Err(ProbeError::SkippedStructRef { struct_name }) => {
                self.warns
                    .push(warn_skipped_struct_ref(struct_name).attach_span(&field_item.span));
                Err(ProbeError::Suppressed)
            }
            Err(ProbeError::UnsupportedType { reason, .. })
            | Err(ProbeError::UnsupportedImport { path: reason, .. }) => {
                self.warns
                    .push(warn_unsupported_type(reason).attach_span(&field_item.span));
                Err(ProbeError::Suppressed)
            }
        }
    }

    fn intern_fn(&mut self, id: RId) -> Result<Option<AsgId>, ProbeError> {
        if let Some(slot) = self.fn_intern.get(&id) {
            return Ok(*slot);
        }
        let item = self.get_item(id)?;
        let func = match &item.inner {
            ItemEnum::Function(f) => f,
            _ => {
                return Err(ProbeError::UnsupportedType {
                    reason: "expected function".into(),
                });
            }
        };

        let span = item.span.as_ref().map(convert_span);
        let path = path_of(item, id, self.crate_);

        // Dry-probe signature.
        let probed: Result<(Option<Receiver>, Vec<FnParam>, Type), ProbeError> = (|| {
            let mut receiver = None;
            let mut params = Vec::new();
            for (pname, ty) in &func.sig.inputs {
                if pname == "self" {
                    receiver = Some(receiver_of(ty));
                    continue;
                }
                let (mode, underlying) = param_mode_and_type(ty);
                params.push(FnParam {
                    name: pname.clone(),
                    mode,
                    ty: self.resolve_type(underlying)?,
                });
            }
            let return_type = match &func.sig.output {
                Some(ty) => self.resolve_type(ty)?,
                None => Type::Primitive(Primitive::Unit),
            };
            Ok((receiver, params, return_type))
        })();

        let (receiver, params, return_type) = match probed {
            Ok(p) => p,
            Err(ProbeError::MissingItem(rid)) => return Err(ProbeError::MissingItem(rid)),
            Err(ProbeError::Suppressed) => {
                self.fn_intern.insert(id, None);
                return Ok(None);
            }
            Err(ProbeError::SkippedStructRef { struct_name }) => {
                self.warns
                    .push(warn_skipped_struct_ref(struct_name).attach_span(&item.span));
                self.fn_intern.insert(id, None);
                return Ok(None);
            }
            Err(ProbeError::UnsupportedType { reason, .. })
            | Err(ProbeError::UnsupportedImport { path: reason, .. }) => {
                self.warns
                    .push(warn_unsupported_type(reason).attach_span(&item.span));

                self.fn_intern.insert(id, None);
                return Ok(None);
            }
        };

        let fid = AsgId(self.asg.items.len());
        self.fn_intern.insert(id, Some(fid));
        self.asg.items.push(Item::Fn(FnDef {
            id: fid,
            path,
            span,
            receiver,
            params,
            return_type,
            docs: item.docs.clone(),
        }));
        Ok(Some(fid))
    }

    fn resolve_type(&mut self, ty: &RType) -> Result<Type, ProbeError> {
        match ty {
            RType::Primitive(name) => {
                primitive_to_type(name).ok_or_else(|| ProbeError::UnsupportedType {
                    reason: format!("unsupported primitive \"{name}\""),
                })
            }
            RType::Tuple(ts) if ts.is_empty() => Ok(Type::Primitive(Primitive::Unit)),
            RType::Tuple(_) => Err(ProbeError::UnsupportedType {
                reason: "tuples are not supported".into(),
            }),
            RType::Slice(_) => Err(ProbeError::UnsupportedType {
                reason: "slices are not supported".into(),
            }),
            RType::Array { .. } => Err(ProbeError::UnsupportedType {
                reason: "fixed-size arrays are not supported".into(),
            }),
            RType::BorrowedRef { type_, .. } => self.resolve_type(type_),
            RType::ResolvedPath(path) => self.resolve_path(path),
            RType::Generic(name) if name == "Self" => {
                let Some(&self_id) = self.self_stack.last() else {
                    return Err(ProbeError::UnsupportedType {
                        reason: "Self outside of impl context".into(),
                    });
                };
                match self.intern_struct(self_id)? {
                    Some(sid) => Ok(Type::Struct(sid)),
                    None => {
                        let struct_name = self
                            .crate_
                            .index
                            .get(&self_id)
                            .and_then(|it| it.name.clone())
                            .unwrap_or_else(|| "Self".to_string());
                        Err(ProbeError::SkippedStructRef { struct_name })
                    }
                }
            }
            other => Err(ProbeError::UnsupportedType {
                reason: format!("unsupported type variant: {other:?}"),
            }),
        }
    }

    fn resolve_path(&mut self, path: &RPath) -> Result<Type, ProbeError> {
        if let Some(t) = self.try_well_known(path)? {
            return Ok(t);
        }
        let Some(item) = self.crate_.index.get(&path.id) else {
            if let Some((_, ext_path)) = self.external_import(path.id) {
                return Err(ProbeError::UnsupportedImport { path: ext_path });
            }
            return Err(ProbeError::MissingItem(path.id));
        };
        match &item.inner {
            ItemEnum::Struct(_) => match self.intern_struct(path.id)? {
                Some(sid) => Ok(Type::Struct(sid)),
                None => Err(ProbeError::SkippedStructRef {
                    struct_name: path.path.clone(),
                }),
            },
            _ => Err(ProbeError::UnsupportedType {
                reason: format!("resolved path \"{}\" is not a struct", path.path),
            }),
        }
    }

    fn try_well_known(&mut self, path: &RPath) -> Result<Option<Type>, ProbeError> {
        let Some(summary) = self.crate_.paths.get(&path.id) else {
            return Ok(None);
        };
        let crate_name = self
            .crate_
            .external_crates
            .get(&summary.crate_id)
            .map(|ec| ec.name.as_str());
        if !matches!(crate_name, Some("std" | "core" | "alloc")) {
            return Ok(None);
        }
        let last = summary.path.last().map(String::as_str);
        let out = match last {
            Some("String") => Type::Primitive(Primitive::String),
            _ => {
                return Err(ProbeError::UnsupportedType {
                    reason: format!("unsupported std type \"{}\"", summary.path.join("::")),
                });
            }
        };
        Ok(Some(out))
    }

    fn get_item(&self, id: RId) -> Result<&'a RItem, ProbeError> {
        self.crate_
            .index
            .get(&id)
            .ok_or(ProbeError::MissingItem(id))
    }

    fn external_import(&self, id: RId) -> Option<(String, String)> {
        let summary = self.crate_.paths.get(&id)?;
        let ext = self.crate_.external_crates.get(&summary.crate_id)?;
        Some((ext.name.clone(), summary.path.join("::")))
    }
}

fn is_public(item: &RItem) -> bool {
    matches!(item.visibility, Visibility::Public)
        || matches!(item.inner, ItemEnum::Use(_) | ItemEnum::Module(_))
}

fn primitive_to_type(name: &str) -> Option<Type> {
    let prim = match name {
        "bool" => Primitive::Boolean,
        "char" => Primitive::Char,
        "str" => Primitive::Str,
        "i8" => Primitive::i8,
        "i16" => Primitive::i16,
        "i32" => Primitive::i32,
        "i64" => Primitive::i64,
        "u8" => Primitive::u8,
        "u16" => Primitive::u16,
        "u32" => Primitive::u32,
        "u64" => Primitive::u64,
        "isize" => Primitive::isize,
        "usize" => Primitive::usize,
        "f32" => Primitive::f32,
        "f64" => Primitive::f64,
        _ => return None,
    };
    Some(Type::Primitive(prim))
}

fn param_mode_and_type(ty: &RType) -> (ParamMode, &RType) {
    match ty {
        RType::BorrowedRef {
            is_mutable, type_, ..
        } => (
            if *is_mutable {
                ParamMode::ByMut
            } else {
                ParamMode::ByRef
            },
            type_,
        ),
        _ => (ParamMode::ByValue, ty),
    }
}

/// Determine the `self` receiver mode from the type of a `self` input.
fn receiver_of(ty: &RType) -> Receiver {
    match ty {
        RType::BorrowedRef {
            is_mutable: true, ..
        } => Receiver::ByMut,
        RType::BorrowedRef {
            is_mutable: false, ..
        } => Receiver::ByRef,
        _ => Receiver::ByValue,
    }
}

/// Recognize a std/core/alloc marker trait by identity.
fn recognize_well_known(crate_: &Crate, trait_path: &RPath) -> Option<WellKnownTrait> {
    if let Some(summary) = crate_.paths.get(&trait_path.id) {
        let crate_name = crate_
            .external_crates
            .get(&summary.crate_id)
            .map(|ec| ec.name.as_str());
        if matches!(crate_name, Some("std" | "core" | "alloc"))
            && summary.path.last().map(String::as_str) == Some("Clone")
        {
            return Some(WellKnownTrait::Clone);
        }
        return None;
    }
    if trait_path.path.rsplit("::").next() == Some("Clone") {
        return Some(WellKnownTrait::Clone);
    }
    None
}

pub(crate) fn convert_span(s: &RSpan) -> Span {
    Span {
        filename: s.filename.clone(),
        begin: s.begin,
        end: s.end,
    }
}

fn path_of(item: &RItem, id: RId, crate_: &Crate) -> ItemPath {
    if let Some(summary) = crate_.paths.get(&id) {
        return ItemPath::new(summary.path.clone());
    }
    let name = item.name.clone().unwrap_or_default();
    ItemPath::new(vec![name])
}

#[derive(Debug)]
enum ProbeError {
    MissingItem(RId),
    UnsupportedType { reason: String },
    UnsupportedImport { path: String },
    SkippedStructRef { struct_name: String },
    Suppressed,
}

fn warn_unsupported_enum() -> Warn {
    warn_unsupported_type("enums are not supported".into())
}

fn warn_unsupported_type(reason: String) -> Warn {
    Warn::new("rust-curse::unsupported_type", reason)
}

fn warn_skipped_struct_ref(struct_name: String) -> Warn {
    Warn::new(
        "rust-curse::skipped_struct_ref",
        format!("references a skipped struct \"{struct_name}\""),
    )
    .help("fix the warning emitted for this struct to expose it")
}

fn warn_unsupported_import(crate_name: String, path: String) -> Warn {
    Warn::new(
        "rust-curse::unsupported_import",
        format!("import from external crate \"{crate_name}\": \"{path}\""),
    )
}

impl Warn {
    pub(self) fn attach_span(self, span: &Option<rustdoc_types::Span>) -> Self {
        match span {
            Some(span) => self
                .label(WarnLabel::new(None, span.begin, span.end))
                .source_code(&span.filename),
            None => self,
        }
    }
}
