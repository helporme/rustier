//! Abstract Syntax Graph (ASG) for rust code.
//!
//! In essence it's a subset of rustdoc types with salsa annotations. Lowering
//! from the rustdoc is implemented in [`from_rustdoc`].

#![allow(non_camel_case_types)]

use std::path::PathBuf;

/// Complete path to an item as it appears in the source crate,
/// e.g. `crate::module::Type` or a
/// `external_crate::module::Type` for external crate.
#[salsa::interned(debug)]
pub struct ItemPath<'db> {
    #[returns(ref)]
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
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

/// A type that rust-curse treats as a primitive.
#[derive(Debug, Clone, Eq, PartialEq, salsa::Update)]
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

#[derive(Debug, Clone, Eq, PartialEq, salsa::Update)]
pub enum Type<'db> {
    Primitive(Primitive),
    Struct(StructDef<'db>),
    Enum(EnumDef<'db>),
}

/// A parameter mode.
#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
pub enum ParamMode {
    ByValue,
    ByRef,
    ByMut,
}

/// The `self` receiver of an associated fn.
#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
pub enum Receiver {
    /// `self`
    ByValue,
    /// `&self`
    ByRef,
    /// `&mut self`
    ByMut,
}

#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
pub struct FnParam<'db> {
    pub name: String,
    pub mode: ParamMode,
    pub ty: Type<'db>,
}

#[salsa::tracked(debug)]
pub struct FnDef<'db> {
    pub path: ItemPath<'db>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
    #[tracked]
    pub receiver: Option<Receiver>,
    #[tracked]
    #[returns(ref)]
    pub params: Vec<FnParam<'db>>,
    #[tracked]
    #[returns(ref)]
    pub return_type: Type<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

/// A struct field.
#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
pub struct StructField<'db> {
    pub name: String,
    pub ty: Type<'db>,
    pub span: Option<Span>,
    pub docs: Option<String>,
}

/// Shape of a struct or enum variant payload.
#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
pub enum StructShape<'db> {
    Unit,
    Tuple(Vec<Type<'db>>),
    Struct(Vec<StructField<'db>>),
}

#[salsa::tracked(debug)]
pub struct StructDef<'db> {
    pub path: ItemPath<'db>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
    #[tracked]
    #[returns(ref)]
    pub shape: StructShape<'db>,
    #[tracked]
    #[returns(ref)]
    pub impls: Vec<ImplDef<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    pub has_private_fields: bool,
}

/// A variant of an enum.
#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
pub struct EnumVariant<'db> {
    pub name: String,
    pub shape: StructShape<'db>,
    pub span: Option<Span>,
    pub docs: Option<String>,
}

/// An enum definition (at least one struct-shaped variant -> data enum).
#[salsa::tracked(debug)]
pub struct EnumDef<'db> {
    pub path: ItemPath<'db>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
    #[tracked]
    #[returns(ref)]
    pub variants: Vec<EnumVariant<'db>>,
    #[tracked]
    #[returns(ref)]
    pub impls: Vec<ImplDef<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

#[salsa::tracked(debug)]
pub struct TraitDef<'db> {
    pub path: ItemPath<'db>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
    /// Required fns.
    #[tracked]
    #[returns(ref)]
    pub fns: Vec<FnDef<'db>>,
    /// Impls of this trait.
    #[tracked]
    #[returns(ref)]
    pub impls: Vec<ImplDef<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, salsa::Update)]
pub enum ImplFor<'db> {
    Struct(StructDef<'db>),
    Enum(EnumDef<'db>),
}

#[salsa::tracked(debug)]
pub struct ImplDef<'db> {
    /// Struct or enum this impl is for.
    pub for_: ImplFor<'db>,
    /// `None` for an inherent impl.
    pub trait_: Option<TraitDef<'db>>,
    /// Associated fns this impl defines.
    #[tracked]
    #[returns(ref)]
    pub fns: Vec<FnDef<'db>>,
}

#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
pub enum Item<'db> {
    Mod(ModDef<'db>),
    Struct(StructDef<'db>),
    Enum(EnumDef<'db>),
    Trait(TraitDef<'db>),
    Fn(FnDef<'db>),
}

/// A name brought into a module's public surface.
#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
pub struct Use<'db> {
    pub name: String,
    pub target: Item<'db>,
}

/// A module definition.
#[salsa::tracked(debug)]
pub struct ModDef<'db> {
    pub path: ItemPath<'db>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
    /// Items brought into this module's public surface.
    #[tracked]
    #[returns(ref)]
    pub uses: Vec<Use<'db>>,
    /// Public items defined in this module.
    #[tracked]
    #[returns(ref)]
    pub items: Vec<Item<'db>>,
}

pub type Asg<'db> = ModDef<'db>;
