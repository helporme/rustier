//! ASG for rust.

#![allow(non_camel_case_types)]

pub mod from_rustdoc;

use std::path::PathBuf;

/// A unique identifier for an item in the ASG.
#[derive(Copy, Clone, Default, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AsgId {
    /// Position in [`Asg::items`].
    pub index: usize,
    /// Stable hash of the item's source identity.
    pub stable: u64,
}

impl AsgId {
    pub fn new(index: usize, path: &ItemPath) -> Self {
        Self {
            index,
            stable: fnv1a(path.segments.join("::").as_bytes()),
        }
    }

    pub fn of_impl(
        index: usize,
        for_: AsgId,
        trait_: Option<&TraitRef>,
        module: &[String],
    ) -> Self {
        let mut hash = fnv1a_step(FNV_OFFSET_BASIS, b"impl:");
        hash = fnv1a_step(hash, &for_.stable.to_le_bytes());
        match trait_ {
            None => hash = fnv1a_step(hash, b"|inherent"),
            Some(TraitRef::Local(tid)) => {
                hash = fnv1a_step(hash, b"|trait:");
                hash = fnv1a_step(hash, &tid.stable.to_le_bytes());
            }
            Some(TraitRef::WellKnown(wk)) => {
                hash = fnv1a_step(hash, b"|well-known:");
                hash = fnv1a_step(hash, &[*wk as u8]);
            }
        }
        for segment in module {
            hash = fnv1a_step(hash, b"|");
            hash = fnv1a_step(hash, segment.as_bytes());
        }
        Self {
            index,
            stable: hash,
        }
    }
}

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn fnv1a(bytes: &[u8]) -> u64 {
    fnv1a_step(FNV_OFFSET_BASIS, bytes)
}

fn fnv1a_step(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Complete path to an item as it appears in the source crate,
/// e.g. `crate::module::Type` is stored as `["crate", "module", "Type"]`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct ItemPath {
    pub segments: Vec<String>,
}

impl ItemPath {
    pub fn new(segments: Vec<String>) -> Self {
        Self { segments }
    }

    pub fn name(&self) -> Option<&str> {
        self.segments.last().map(String::as_str)
    }
}

/// Renders `path` as a Rust source path string. If the first segment
/// matches `local_crate`, it's rewritten to `crate` so the emitted code
/// is valid from inside that crate; otherwise segments are joined as-is,
/// preserving extern-crate references.
pub fn rust_path(path: &ItemPath, local_crate: &str) -> String {
    match path.segments.split_first() {
        Some((head, tail)) if head == local_crate => {
            if tail.is_empty() {
                "crate".to_string()
            } else {
                format!("crate::{}", tail.join("::"))
            }
        }
        _ => path.segments.join("::"),
    }
}

/// A structural type term. Nominal defs are referenced by id.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Primitive(Primitive),
    Struct(AsgId),
    Enum(AsgId),
}

/// A primitive type (everything that isn't a nominal struct).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Primitive {
    Unit,
    Boolean,
    Char,
    String,
    Str,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    i64,
    u64,
    isize,
    usize,
    f32,
    f64,
}

/// A function definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnDef {
    pub id: AsgId,
    pub path: ItemPath,
    pub span: Option<Span>,
    pub receiver: Option<Receiver>,
    pub params: Vec<FnParam>,
    pub return_type: Type,
    pub docs: Option<String>,
}

/// The `self` receiver of an associated fn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Receiver {
    /// `self`
    ByValue,
    /// `&self`
    ByRef,
    /// `&mut self`
    ByMut,
}

/// A function parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnParam {
    pub name: String,
    pub mode: ParamMode,
    pub ty: Type,
}

/// A parameter mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamMode {
    ByValue,
    ByRef,
    ByMut,
}

/// A struct definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructDef {
    pub id: AsgId,
    pub path: ItemPath,
    pub span: Option<Span>,
    pub shape: StructShape,
    pub impls: Vec<AsgId>,
    pub docs: Option<String>,
    pub has_private_fields: bool,
}

/// Shape of a struct or enum variant payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructShape {
    Unit,
    Tuple(Vec<Type>),
    Struct(Vec<StructField>),
}

/// A struct field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructField {
    pub name: String,
    pub ty: Type,
    pub span: Option<Span>,
    pub docs: Option<String>,
}

/// An enum definition (at least one struct-shaped variant -> data enum).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumDef {
    pub id: AsgId,
    pub path: ItemPath,
    pub span: Option<Span>,
    pub variants: Vec<EnumVariant>,
    pub impls: Vec<AsgId>,
    pub docs: Option<String>,
}

/// A variant of an enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumVariant {
    pub name: String,
    pub shape: StructShape,
    pub span: Option<Span>,
    pub docs: Option<String>,
}

/// An `impl` block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImplDef {
    pub id: AsgId,
    /// Struct or enum this impl is for.
    pub for_: AsgId,
    /// `None` for an inherent impl.
    pub trait_: Option<TraitRef>,
    /// Associated fns this impl defines.
    pub fns: Vec<AsgId>,
}

/// How an impl names the trait it implements.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraitRef {
    /// A trait defined in the crate.
    Local(AsgId),
    /// A std/core trait we recognize for internal purposes (e.g. `Clone`).
    WellKnown(WellKnownTrait),
}

/// A std/core trait recognized by identity during ASG build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WellKnownTrait {
    Clone,
}

/// A trait definition.
///
/// Traits with assoc consts, assoc types, or any generics are not represented.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraitDef {
    pub id: AsgId,
    pub path: ItemPath,
    pub span: Option<Span>,
    /// Required fns.
    pub fns: Vec<AsgId>,
    /// Impls of this trait.
    pub impls: Vec<AsgId>,
    pub docs: Option<String>,
}

/// A module definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModDef {
    pub id: AsgId,
    pub path: ItemPath,
    /// Source span where this module is declared.
    pub span: Option<Span>,
    pub uses: Vec<Use>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    /// `filename` interpretation depends on the originating crate: relative to
    /// the project root for the local crate, absolute under `~/.cargo/...` for
    /// registry deps, and so on. `None` if there is no source span available.
    pub filename: PathBuf,
    /// 1-indexed `(line, column)` begin/end of the span.
    pub begin: (usize, usize),
    /// 1-indexed `(line, column)` begin/end of the span.
    pub end: (usize, usize),
}

/// A name brought into a module's public surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Use {
    pub name: String,
    pub target: AsgId,
}

/// An item in the ASG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Struct(StructDef),
    Enum(EnumDef),
    Fn(FnDef),
    Mod(ModDef),
    Impl(ImplDef),
    Trait(TraitDef),
}

/// The rust curse ASG for a Rust crate.
#[derive(Clone, Debug, Default)]
pub struct Asg {
    pub root: AsgId,
    pub crate_name: String,
    pub items: Vec<Item>,
}

impl Asg {
    /// Gets an item by its ID, panics if the item is not found.
    pub fn get_unchecked(&self, id: AsgId) -> &Item {
        &self.items[id.index]
    }

    /// Mutably gets an item by its ID, panics if the item is not found.
    pub fn get_mut_unchecked(&mut self, id: AsgId) -> &mut Item {
        &mut self.items[id.index]
    }

    /// Gets a struct by its ID, panics if the item is not a struct.
    pub fn get_struct_unchecked(&self, id: AsgId) -> &StructDef {
        match self.get_unchecked(id) {
            Item::Struct(s) => s,
            other => panic!("expected struct at {id:?}, got {other:?}"),
        }
    }

    /// Gets an enum by its ID, panics if the item is not an enum.
    pub fn get_enum_unchecked(&self, id: AsgId) -> &EnumDef {
        match self.get_unchecked(id) {
            Item::Enum(e) => e,
            other => panic!("expected enum at {id:?}, got {other:?}"),
        }
    }

    /// Gets a function by its ID, panics if the item is not a function.
    pub fn get_fn_unchecked(&self, id: AsgId) -> &FnDef {
        match self.get_unchecked(id) {
            Item::Fn(f) => f,
            other => panic!("expected fn at {id:?}, got {other:?}"),
        }
    }

    /// Gets a module by its ID, panics if the item is not a module.
    pub fn get_mod_unchecked(&self, id: AsgId) -> &ModDef {
        match self.get_unchecked(id) {
            Item::Mod(m) => m,
            other => panic!("expected mod at {id:?}, got {other:?}"),
        }
    }

    /// Gets an impl by its ID, panics if the item is not an impl.
    pub fn get_impl_unchecked(&self, id: AsgId) -> &ImplDef {
        match self.get_unchecked(id) {
            Item::Impl(i) => i,
            other => panic!("expected impl at {id:?}, got {other:?}"),
        }
    }

    /// Gets a trait by its ID, panics if the item is not a trait.
    pub fn get_trait_unchecked(&self, id: AsgId) -> &TraitDef {
        match self.get_unchecked(id) {
            Item::Trait(t) => t,
            other => panic!("expected trait at {id:?}, got {other:?}"),
        }
    }
}
