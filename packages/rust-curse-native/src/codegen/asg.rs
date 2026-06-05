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
pub struct Path<'db> {
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

#[derive(Debug, Clone, Eq, PartialEq, salsa::Update)]
pub enum Primitive {
    Boolean,
    Char,
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
    Ref {
        is_mutable: bool,
        ty: Box<Type<'db>>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
pub struct FnParam<'db> {
    pub name: String,
    pub ty: Type<'db>,
}

#[salsa::tracked(debug)]
pub struct FnDef<'db> {
    pub path: Path<'db>,
    /// Whether the function has a body.
    ///
    /// For example, it is `true` for functions declared for trait.
    /// ```
    #[tracked]
    pub has_body: bool,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
    #[tracked]
    #[returns(ref)]
    pub params: Vec<FnParam<'db>>,
    #[tracked]
    #[returns(ref)]
    pub return_type: Option<Type<'db>>,
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
    /// Unit struct like `struct Foo;`.
    // Unit,
    /// Tuple struct like `struct Foo(Bar, Baz)`.
    /// Private fields are None.
    // Tuple(Vec<Option<TupleField<'db>>>),
    /// Plain struct like `struct Foo { bar: Bar }`.
    Plain {
        fields: Vec<StructField<'db>>,
        /// Whether the struct has also private fields.
        // todo: check the #[docs(hidden)] influence on private fields
        has_stripped_fields: bool,
    },
}

#[salsa::tracked(debug)]
pub struct StructDef<'db> {
    pub path: Path<'db>,
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
    pub path: Path<'db>,
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
    pub path: Path<'db>,
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

#[salsa::tracked(debug)]
pub struct ImplDef<'db> {
    /// Struct or enum this impl is for.
    // Contains a path to avoid circular references when building the ASG.
    pub for_: Path<'db>,
    /// `None` for an inherent impl, path to the trait this impl implements
    /// otherwise.
    // Contains a path to avoid circular references when building the ASG.
    pub trait_: Option<Path<'db>>,
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
    /// The name this use is given in the module's public surface.
    ///
    /// For example, if this use is `use foo::bar as baz`, then `name` would be
    /// `"baz"`.
    pub name: String,
    pub item: Item<'db>,
}

/// A module definition.
#[salsa::tracked(debug)]
pub struct ModDef<'db> {
    pub path: Path<'db>,
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
