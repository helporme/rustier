//! Creates rust-curse ASG from rustdoc.

use std::collections::HashMap;
use std::collections::HashSet;

use rustdoc_types::{
    Crate, GenericArg, GenericArgs, Id as RId, Item as RItem, ItemEnum, Path as RPath,
    Span as RSpan, StructKind, Type as RType, Use, Visibility,
};

use super::{
    Asg, AsgId, Ctor, Export, FnDef, FnParam, Item, ItemPath, ModDef, ParamMode, Primitive, Span,
    StructDef, StructField, StructShape, Type,
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
        let mut exports = Vec::new();

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
                        exports.push(Export {
                            name: child_name,
                            target: child_mid,
                        });
                    }
                }
                ItemEnum::Struct(_) => {
                    if let Some(sid) = self.intern_struct(*child_id)? {
                        exports.push(Export {
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
                        exports.push(Export {
                            name: child_name,
                            target: fid,
                        });
                    }
                }
                ItemEnum::Use(use_) => {
                    let mut visited = HashSet::new();
                    self.add_use(use_, &mut exports, &mut visited, child)?;
                }
                _ => {}
            }
        }

        if exports.is_empty() && mod_id != self.crate_.root {
            self.asg.items.pop();
            self.mod_intern.remove(&mod_id);
            return Ok(None);
        }

        if let Item::Mod(m) = &mut self.asg.items[mid.0 as usize] {
            m.exports = exports;
        }
        Ok(Some(mid))
    }

    fn add_use(
        &mut self,
        use_: &Use,
        exports: &mut Vec<Export>,
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
                            exports.push(Export {
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
                            exports.push(Export {
                                name: child_name,
                                target: fid,
                            });
                        }
                    }
                    ItemEnum::Module(_) => {
                        if let Some(mid) = self.build_module(child_id)? {
                            exports.push(Export {
                                name: child_name,
                                target: mid,
                            });
                        }
                    }
                    ItemEnum::Use(inner_use) => {
                        self.add_use(inner_use, exports, visited, child)?;
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
                    exports.push(Export { name, target: sid });
                }
            }
            ItemEnum::Enum(_) if is_public(target) => {
                self.warns
                    .push(warn_unsupported_enum().attach_span(&target.span));
            }
            ItemEnum::Function(_) if is_public(target) => {
                if let Some(fid) = self.intern_fn(tid)? {
                    exports.push(Export { name, target: fid });
                }
            }
            ItemEnum::Module(_) => {
                if let Some(target_mid) = self.build_module(tid)? {
                    exports.push(Export {
                        name,
                        target: target_mid,
                    });
                }
            }
            ItemEnum::Use(inner_use) => {
                self.add_use(inner_use, exports, visited, target)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn reserve_mod(&mut self, path: ItemPath, span: Option<Span>) -> AsgId {
        let id = AsgId(self.asg.items.len() as u32);
        self.asg.items.push(Item::Mod(ModDef {
            path,
            span,
            exports: Vec::new(),
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

        let clonable = has_clone_impl(&struct_.impls, self.crate_);

        // Shape is good. Commit slot before computing ctor (ctor may
        // recurse back via Self).
        let sid = AsgId(self.asg.items.len() as u32);
        self.struct_intern.insert(id, Some(sid));
        self.asg.items.push(Item::Struct(StructDef {
            path,
            span,
            ctor: Ctor::Implicit,
            clonable,
            shape,
            docs: item.docs.clone(),
        }));

        let ctor = self.find_ctor(&struct_.impls, id, item)?;

        if let Item::Struct(def) = &mut self.asg.items[sid.0 as usize] {
            def.ctor = ctor;
        }
        Ok(Some(sid))
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

    fn find_ctor(
        &mut self,
        impls: &[RId],
        self_id: RId,
        self_item: &RItem,
    ) -> Result<Ctor, ProbeError> {
        struct Candidate {
            key: Vec<String>,
            impl_id_ord: u32,
            assoc_id: RId,
        }
        let mut candidates: Vec<Candidate> = Vec::new();

        for impl_id in impls {
            let Some(impl_item) = self.crate_.index.get(impl_id) else {
                continue;
            };
            let ItemEnum::Impl(impl_) = &impl_item.inner else {
                continue;
            };
            for assoc_id in &impl_.items {
                let Some(assoc) = self.crate_.index.get(assoc_id) else {
                    continue;
                };
                if assoc.name.as_deref() != Some("new") {
                    continue;
                }
                let ItemEnum::Function(func) = &assoc.inner else {
                    continue;
                };
                if func.sig.inputs.iter().any(|(n, _)| n == "self") {
                    continue;
                }
                if !returns_self(func.sig.output.as_ref(), self_id) {
                    continue;
                }
                let mut key: Vec<String> = match &impl_.trait_ {
                    Some(trait_path) => match self.crate_.paths.get(&trait_path.id) {
                        Some(summary) => summary.path.clone(),
                        None => trait_path.path.split("::").map(str::to_string).collect(),
                    },
                    None => path_of(self_item, self_id, self.crate_).segments,
                };
                key.push("new".to_string());
                candidates.push(Candidate {
                    key,
                    impl_id_ord: impl_id.0,
                    assoc_id: *assoc_id,
                });
            }
        }

        candidates.sort_by(|a, b| a.key.cmp(&b.key).then(a.impl_id_ord.cmp(&b.impl_id_ord)));

        let Some(chosen) = candidates.into_iter().next() else {
            return Ok(Ctor::Implicit);
        };

        self.self_stack.push(self_id);
        let res = self.intern_fn(chosen.assoc_id);
        self.self_stack.pop();

        match res? {
            Some(fid) => Ok(Ctor::Explicit(fid)),
            None => Ok(Ctor::Implicit),
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
        let probed: Result<(Vec<FnParam>, Type), ProbeError> = (|| {
            let mut params = Vec::new();
            for (pname, ty) in &func.sig.inputs {
                if pname == "self" {
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
            Ok((params, return_type))
        })();

        let (params, return_type) = match probed {
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

        let fid = AsgId(self.asg.items.len() as u32);
        self.fn_intern.insert(id, Some(fid));
        self.asg.items.push(Item::Fn(FnDef {
            path,
            span,
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

fn returns_self(output: Option<&RType>, self_id: RId) -> bool {
    let Some(ty) = output else { return false };
    match ty {
        RType::Generic(name) if name == "Self" => true,
        RType::ResolvedPath(p) if p.id == self_id => true,
        RType::ResolvedPath(p) if is_result_path(p) => {
            first_type_arg(p).is_some_and(|t| returns_self(Some(t), self_id))
        }
        _ => false,
    }
}

fn is_result_path(p: &RPath) -> bool {
    p.path.rsplit("::").next() == Some("Result")
}

fn first_type_arg(p: &RPath) -> Option<&RType> {
    let GenericArgs::AngleBracketed { args, .. } = p.args.as_deref()? else {
        return None;
    };
    args.iter().find_map(|a| match a {
        GenericArg::Type(t) => Some(t),
        _ => None,
    })
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

fn has_clone_impl(impls: &[RId], crate_: &Crate) -> bool {
    impls.iter().any(|impl_id| {
        let Some(impl_item) = crate_.index.get(impl_id) else {
            return false;
        };
        let ItemEnum::Impl(impl_) = &impl_item.inner else {
            return false;
        };
        if impl_.is_negative {
            return false;
        }
        let Some(trait_path) = impl_.trait_.as_ref() else {
            return false;
        };
        if let Some(summary) = crate_.paths.get(&trait_path.id) {
            let crate_name = crate_
                .external_crates
                .get(&summary.crate_id)
                .map(|ec| ec.name.as_str());
            return matches!(crate_name, Some("std" | "core" | "alloc"))
                && summary.path.last().map(String::as_str) == Some("Clone");
        }
        trait_path.path.rsplit("::").next() == Some("Clone")
    })
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
