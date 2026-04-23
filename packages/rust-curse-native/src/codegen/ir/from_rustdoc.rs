//! Creates rust-curse IR from rustdoc.

use std::collections::HashMap;
use std::collections::HashSet;

use rustdoc_types::{
    Crate, GenericArg, GenericArgs, Id, Item, ItemEnum, Path, StructKind, Type as RType, Use,
    Visibility,
};

use super::{
    Ctor, Export, ExportKind, FnDef, FnId, FnParam, ItemPath, ModDef, ModId, NumberType, ParamMode,
    StructBoundary, StructDef, StructField, StructId, StructShape, Type, TypeId, IR,
};

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("item {0:?} not found in crate index")]
    MissingItem(Id),
    #[error("unsupported type at {id:?}: {reason}")]
    UnsupportedType { id: Id, reason: String },
    #[error("array length {0:?} is not a usize")]
    BadArrayLen(String),
}

pub fn build_ir(allowed: &HashSet<Id>, crate_: &Crate) -> Result<IR, BuildError> {
    let mut b = Builder {
        crate_,
        ir: IR::default(),
        type_intern: HashMap::new(),
        struct_intern: HashMap::new(),
        fn_intern: HashMap::new(),
        mod_intern: HashMap::new(),
        self_stack: Vec::new(),
    };

    let root = b
        .build_module(crate_.root, allowed)?
        .expect("root module has no id reservation");
    b.ir.root = root;
    Ok(b.ir)
}

struct Builder<'a> {
    crate_: &'a Crate,
    ir: IR,
    type_intern: HashMap<Type, TypeId>,
    struct_intern: HashMap<Id, StructId>,
    fn_intern: HashMap<Id, FnId>,
    mod_intern: HashMap<Id, ModId>,
    self_stack: Vec<Id>,
}

impl<'a> Builder<'a> {
    fn build_module(
        &mut self,
        mod_id: Id,
        allowed: &HashSet<Id>,
    ) -> Result<Option<ModId>, BuildError> {
        if let Some(&mid) = self.mod_intern.get(&mod_id) {
            return Ok(Some(mid));
        }
        let item = self.get_item(mod_id)?;
        let module = match &item.inner {
            ItemEnum::Module(m) => m,
            _ => return Ok(None),
        };

        let mid = self.reserve_mod(path_of(item, mod_id, self.crate_));
        self.mod_intern.insert(mod_id, mid);
        let mut exports = Vec::new();

        for child_id in &module.items {
            let Some(child) = self.crate_.index.get(child_id) else {
                continue;
            };
            let child_name = child.name.clone().unwrap_or_default();
            match &child.inner {
                ItemEnum::Module(_) => {
                    if let Some(child_mid) = self.build_module(*child_id, allowed)? {
                        exports.push(Export {
                            name: child_name,
                            kind: ExportKind::Mod(child_mid),
                        });
                    }
                }
                ItemEnum::Struct(_) if allowed.contains(child_id) => {
                    let sid = self.intern_struct(*child_id)?;
                    exports.push(Export {
                        name: child_name,
                        kind: ExportKind::Struct(sid),
                    });
                }
                ItemEnum::Enum(_) if allowed.contains(child_id) => {
                    return Err(BuildError::UnsupportedType {
                        id: *child_id,
                        reason: "enums are not supported".into(),
                    });
                }
                ItemEnum::Function(_) if allowed.contains(child_id) => {
                    let fid = self.intern_fn(*child_id)?;
                    exports.push(Export {
                        name: child_name,
                        kind: ExportKind::Fn(fid),
                    });
                }
                ItemEnum::Use(use_) => {
                    let mut visited = HashSet::new();
                    self.add_use(use_, allowed, &mut exports, &mut visited)?;
                }
                _ => {}
            }
        }

        if exports.is_empty() && mod_id != self.crate_.root {
            // Module produced nothing -> remove from IR and intern map.
            // Safe to pop because nothing else can reference this ModId yet.
            self.ir.mods.pop();
            self.mod_intern.remove(&mod_id);
            return Ok(None);
        }

        self.ir.mods[mid.0 as usize].exports = exports;
        Ok(Some(mid))
    }

    fn add_use(
        &mut self,
        use_: &Use,
        allowed: &HashSet<Id>,
        exports: &mut Vec<Export>,
        visited: &mut HashSet<Id>,
    ) -> Result<(), BuildError> {
        let Some(tid) = use_.id else { return Ok(()) };
        let Some(target) = self.crate_.index.get(&tid) else {
            return Ok(());
        };

        if use_.is_glob {
            if !visited.insert(tid) {
                return Ok(());
            }
            let ItemEnum::Module(source_mod) = &target.inner else {
                return Ok(());
            };
            let child_ids: Vec<Id> = source_mod.items.clone();
            for child_id in child_ids {
                let Some(child) = self.crate_.index.get(&child_id) else {
                    continue;
                };
                let child_name = child.name.clone().unwrap_or_default();
                match &child.inner {
                    ItemEnum::Struct(_) if allowed.contains(&child_id) => {
                        let sid = self.intern_struct(child_id)?;
                        exports.push(Export {
                            name: child_name,
                            kind: ExportKind::Struct(sid),
                        });
                    }
                    ItemEnum::Enum(_) if allowed.contains(&child_id) => {
                        return Err(BuildError::UnsupportedType {
                            id: child_id,
                            reason: "enums are not supported".into(),
                        });
                    }
                    ItemEnum::Function(_) if allowed.contains(&child_id) => {
                        let fid = self.intern_fn(child_id)?;
                        exports.push(Export {
                            name: child_name,
                            kind: ExportKind::Fn(fid),
                        });
                    }
                    ItemEnum::Module(_) => {
                        if let Some(mid) = self.build_module(child_id, allowed)? {
                            exports.push(Export {
                                name: child_name,
                                kind: ExportKind::Mod(mid),
                            });
                        }
                    }
                    ItemEnum::Use(inner_use) => {
                        self.add_use(inner_use, allowed, exports, visited)?;
                    }
                    _ => {}
                }
            }
            return Ok(());
        }

        let name = use_.name.clone();
        match &target.inner {
            ItemEnum::Struct(_) if allowed.contains(&tid) => {
                let sid = self.intern_struct(tid)?;
                exports.push(Export {
                    name,
                    kind: ExportKind::Struct(sid),
                });
            }
            ItemEnum::Enum(_) if allowed.contains(&tid) => {
                return Err(BuildError::UnsupportedType {
                    id: tid,
                    reason: "enums are not supported".into(),
                });
            }
            ItemEnum::Function(_) if allowed.contains(&tid) => {
                let fid = self.intern_fn(tid)?;
                exports.push(Export {
                    name,
                    kind: ExportKind::Fn(fid),
                });
            }
            ItemEnum::Module(_) => {
                if let Some(target_mid) = self.build_module(tid, allowed)? {
                    exports.push(Export {
                        name,
                        kind: ExportKind::Mod(target_mid),
                    });
                }
            }
            ItemEnum::Use(inner_use) => {
                self.add_use(inner_use, allowed, exports, visited)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn reserve_mod(&mut self, path: ItemPath) -> ModId {
        let id = ModId(self.ir.mods.len() as u32);
        self.ir.mods.push(ModDef {
            path,
            exports: Vec::new(),
        });
        id
    }

    fn intern_struct(&mut self, id: Id) -> Result<StructId, BuildError> {
        if let Some(&sid) = self.struct_intern.get(&id) {
            return Ok(sid);
        }
        let item = self.get_item(id)?;
        let struct_ = match &item.inner {
            ItemEnum::Struct(s) => s,
            _ => {
                return Err(BuildError::UnsupportedType {
                    id,
                    reason: "expected struct".into(),
                });
            }
        };

        let sid = StructId(self.ir.structs.len() as u32);
        self.struct_intern.insert(id, sid);
        self.ir.structs.push(StructDef {
            path: path_of(item, id, self.crate_),
            boundary: StructBoundary::Copy,
            shape: StructShape::Unit,
            docs: item.docs.clone(),
        });

        let shape = self.build_struct_shape(&struct_.kind)?;
        let is_copy = can_copy_struct_kind(&struct_.kind, self.crate_);
        let boundary = if is_copy {
            StructBoundary::Copy
        } else {
            let ctor = self.find_ctor(&struct_.impls, id)?;
            StructBoundary::Ref { ctor }
        };

        let def = &mut self.ir.structs[sid.0 as usize];
        def.shape = shape;
        def.boundary = boundary;
        Ok(sid)
    }

    fn build_struct_shape(&mut self, kind: &StructKind) -> Result<StructShape, BuildError> {
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
                    out.push(self.intern_type(ty)?);
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
                        type_id: self.intern_type(ty)?,
                        docs: field_item.docs.clone(),
                    });
                }
                Ok(StructShape::Struct(out))
            }
        }
    }

    fn find_ctor(&mut self, impls: &[Id], self_id: Id) -> Result<Ctor, BuildError> {
        struct Candidate {
            key: Vec<String>,
            impl_id_ord: u32,
            assoc_id: Id,
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
                    None => {
                        let item = self.get_item(self_id)?;
                        path_of(item, self_id, self.crate_).segments
                    }
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
        Ok(Ctor::Explicit(res?))
    }

    fn intern_fn(&mut self, id: Id) -> Result<FnId, BuildError> {
        if let Some(&fid) = self.fn_intern.get(&id) {
            return Ok(fid);
        }
        let item = self.get_item(id)?;
        let func = match &item.inner {
            ItemEnum::Function(f) => f,
            _ => {
                return Err(BuildError::UnsupportedType {
                    id,
                    reason: "expected function".into(),
                });
            }
        };

        let fid = FnId(self.ir.fns.len() as u32);
        self.fn_intern.insert(id, fid);
        self.ir.fns.push(FnDef {
            path: path_of(item, id, self.crate_),
            params: Vec::new(),
            return_type: TypeId::default(),
            docs: item.docs.clone(),
        });

        let mut params = Vec::new();
        for (name, ty) in &func.sig.inputs {
            if name == "self" {
                continue;
            }
            let (mode, underlying) = param_mode_and_type(ty);
            params.push(FnParam {
                name: name.clone(),
                mode,
                type_id: self.intern_type(underlying)?,
            });
        }
        let return_type = match &func.sig.output {
            Some(ty) => self.intern_type(ty)?,
            None => self.intern(Type::Unit),
        };

        let def = &mut self.ir.fns[fid.0 as usize];
        def.params = params;
        def.return_type = return_type;
        Ok(fid)
    }

    fn intern_type(&mut self, ty: &RType) -> Result<TypeId, BuildError> {
        let t = self.resolve_type(ty)?;
        Ok(self.intern(t))
    }

    fn intern(&mut self, ty: Type) -> TypeId {
        if let Some(&id) = self.type_intern.get(&ty) {
            return id;
        }
        let id = TypeId(self.ir.types.len() as u32);
        self.ir.types.push(ty.clone());
        self.type_intern.insert(ty, id);
        id
    }

    fn resolve_type(&mut self, ty: &RType) -> Result<Type, BuildError> {
        match ty {
            RType::Primitive(name) => {
                primitive_to_type(name).ok_or_else(|| BuildError::UnsupportedType {
                    id: Id(0),
                    reason: format!("unsupported primitive `{name}`"),
                })
            }
            RType::Tuple(ts) if ts.is_empty() => Ok(Type::Unit),
            RType::Tuple(_) => Err(BuildError::UnsupportedType {
                id: Id(0),
                reason: "tuples are not supported".into(),
            }),
            RType::Slice(inner) => Ok(Type::Slice(self.intern_type(inner)?)),
            RType::Array { type_, len } => {
                let n = len
                    .parse::<usize>()
                    .map_err(|_| BuildError::BadArrayLen(len.clone()))?;
                Ok(Type::Array(self.intern_type(type_)?, n))
            }
            RType::BorrowedRef { type_, .. } => self.resolve_type(type_),
            RType::ResolvedPath(path) => self.resolve_path(path),
            RType::Generic(name) if name == "Self" => {
                let Some(&self_id) = self.self_stack.last() else {
                    return Err(BuildError::UnsupportedType {
                        id: Id(0),
                        reason: "Self outside of impl context".into(),
                    });
                };
                Ok(Type::Struct(self.intern_struct(self_id)?))
            }
            other => Err(BuildError::UnsupportedType {
                id: Id(0),
                reason: format!("unsupported type variant: {other:?}"),
            }),
        }
    }

    fn resolve_path(&mut self, path: &Path) -> Result<Type, BuildError> {
        if let Some(t) = self.try_well_known(path)? {
            return Ok(t);
        }
        let Some(item) = self.crate_.index.get(&path.id) else {
            return Err(BuildError::MissingItem(path.id));
        };
        match &item.inner {
            ItemEnum::Struct(_) => Ok(Type::Struct(self.intern_struct(path.id)?)),
            _ => Err(BuildError::UnsupportedType {
                id: path.id,
                reason: "resolved path is not a struct".into(),
            }),
        }
    }

    fn try_well_known(&mut self, path: &Path) -> Result<Option<Type>, BuildError> {
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
            Some("String") => Type::String,
            Some("Vec") | Some("VecDeque") | Some("LinkedList") => {
                Type::Vec(self.type_arg(&path.args, 0)?)
            }
            Some("Option") => Type::Option(self.type_arg(&path.args, 0)?),
            Some("Result") => {
                Type::Result(self.type_arg(&path.args, 0)?, self.type_arg(&path.args, 1)?)
            }
            Some("Box" | "Arc" | "Rc" | "Cell" | "RefCell") => {
                let inner = self.type_arg(&path.args, 0)?;
                return Ok(Some(self.ir.types[inner.0 as usize].clone()));
            }
            _ => {
                return Err(BuildError::UnsupportedType {
                    id: path.id,
                    reason: format!("unsupported std type `{}`", summary.path.join("::")),
                });
            }
        };
        Ok(Some(out))
    }

    fn type_arg(
        &mut self,
        args: &Option<Box<GenericArgs>>,
        index: usize,
    ) -> Result<TypeId, BuildError> {
        let Some(args) = args else {
            return Err(BuildError::UnsupportedType {
                id: Id(0),
                reason: "missing generic args".into(),
            });
        };
        let GenericArgs::AngleBracketed { args, .. } = args.as_ref() else {
            return Err(BuildError::UnsupportedType {
                id: Id(0),
                reason: "parenthesized generic args".into(),
            });
        };
        let type_args: Vec<&RType> = args
            .iter()
            .filter_map(|a| match a {
                GenericArg::Type(ty) => Some(ty),
                _ => None,
            })
            .collect();
        let Some(ty) = type_args.get(index) else {
            return Err(BuildError::UnsupportedType {
                id: Id(0),
                reason: format!("missing generic arg at index {index}"),
            });
        };
        self.intern_type(ty)
    }

    fn get_item(&self, id: Id) -> Result<&'a Item, BuildError> {
        self.crate_
            .index
            .get(&id)
            .ok_or(BuildError::MissingItem(id))
    }
}

fn primitive_to_type(name: &str) -> Option<Type> {
    Some(match name {
        "bool" => Type::Boolean,
        "char" => Type::Char,
        "str" => Type::Str,
        "i8" => Type::Number(NumberType::i8),
        "i16" => Type::Number(NumberType::i16),
        "i32" => Type::Number(NumberType::i32),
        "i64" => Type::Number(NumberType::i64),
        "u8" => Type::Number(NumberType::u8),
        "u16" => Type::Number(NumberType::u16),
        "u32" => Type::Number(NumberType::u32),
        "u64" => Type::Number(NumberType::u64),
        "isize" => Type::Number(NumberType::isize),
        "usize" => Type::Number(NumberType::usize),
        "f32" => Type::Number(NumberType::f32),
        "f64" => Type::Number(NumberType::f64),
        _ => return None,
    })
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

fn returns_self(output: Option<&RType>, self_id: Id) -> bool {
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

fn is_result_path(p: &Path) -> bool {
    p.path.rsplit("::").next() == Some("Result")
}

fn first_type_arg(p: &Path) -> Option<&RType> {
    let GenericArgs::AngleBracketed { args, .. } = p.args.as_deref()? else {
        return None;
    };
    args.iter().find_map(|a| match a {
        GenericArg::Type(t) => Some(t),
        _ => None,
    })
}

fn path_of(item: &Item, id: Id, crate_: &Crate) -> ItemPath {
    if let Some(summary) = crate_.paths.get(&id) {
        return ItemPath::new(summary.path.clone());
    }
    let name = item.name.clone().unwrap_or_default();
    ItemPath::new(vec![name])
}

fn can_copy_struct_kind(kind: &StructKind, crate_: &Crate) -> bool {
    match kind {
        StructKind::Plain {
            fields,
            has_stripped_fields,
        } => {
            if *has_stripped_fields {
                return false;
            }
            fields.iter().all(|fid| {
                crate_
                    .index
                    .get(fid)
                    .map(|f| f.visibility == Visibility::Public)
                    .unwrap_or(false)
            })
        }
        _ => false,
    }
}
