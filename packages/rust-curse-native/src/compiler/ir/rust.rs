//! Intermediate representation for the supported subset of Rust's public API.

mod lowering;
pub mod usages;

use crate::compiler::{db::Db, inputs::LocalCrates, mangle::mangle_path, path::FilePath};
use itertools::Either;
use ra_ap_hir::{
    ConstParam as HirConstParam, Crate as HirCrate, Enum as HirEnum, EnumVariant as HirVariant,
    Field as HirField, Function as HirFunction, Impl as HirImpl, LifetimeParam as HirLifetimeParam,
    Module as HirModule, Struct as HirStruct, TypeParam as HirTypeParam,
};
use std::{collections::VecDeque, fmt};
use triomphe::Arc;

/// A root of rust public API.
#[salsa::interned(debug)]
pub struct Rust<'db> {
    #[returns(ref)]
    pub crates: Vec<Crate<'db>>,
}

impl<'db> Rust<'db> {
    pub fn get(db: &'db dyn Db) -> Self {
        Self::lower(db)
    }
}

/// One component of a Rust item path.
#[salsa::interned(debug)]
pub struct PathSegment<'db> {
    #[returns(ref)]
    pub name: String,
}

#[salsa::tracked]
impl<'db> PathSegment<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn mangled(self, db: &'db dyn Db) -> String {
        mangle_path([self.name(db).as_str()])
    }
}

#[salsa::interned(debug)]
pub struct ItemPath<'db> {
    pub crate_: Crate<'db>,
    #[returns(ref)]
    pub segments: Vec<PathSegment<'db>>,
}

#[salsa::tracked]
impl<'db> ItemPath<'db> {
    #[salsa::tracked]
    pub fn from_segment(db: &'db dyn Db, crate_: Crate<'db>, segment: PathSegment<'db>) -> Self {
        Self::new(db, crate_, vec![segment])
    }

    #[salsa::tracked]
    pub fn append(self, db: &'db dyn Db, segment: PathSegment<'db>) -> Self {
        let mut segments = self.segments(db).to_owned();
        segments.push(segment);
        Self::new(db, self.crate_(db), segments)
    }

    #[salsa::tracked(returns(ref))]
    pub fn fqn(self, db: &'db dyn Db) -> String {
        self.segments(db)
            .iter()
            .map(|segment| segment.name(db).as_str())
            .collect::<Vec<_>>()
            .join("::")
    }

    #[salsa::tracked(returns(ref))]
    pub fn mangled(self, db: &'db dyn Db) -> String {
        self.segments(db)
            .iter()
            .map(|segment| segment.mangled(db).as_str())
            .collect::<Vec<_>>()
            .join("_")
    }

    pub fn segment(self, db: &'db dyn Db) -> PathSegment<'db> {
        *self.segments(db).last().expect("Item must have a name")
    }
}

pub trait AsItem<'db>: Sized {
    fn path(self, db: &'db dyn Db) -> ItemPath<'db>;

    fn fqn(self, db: &'db dyn Db) -> &'db str {
        self.path(db).fqn(db)
    }
}

macro_rules! as_item {
    ($type:ident) => {
        impl<'db> AsItem<'db> for $type<'db> {
            fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
                <$type<'db>>::path(self, db)
            }
        }
    };
}

#[salsa::interned(debug)]
pub struct Crate<'db> {
    hir: HirCrate,
}

#[salsa::tracked]
impl<'db> Crate<'db> {
    /// Whether this crate belongs to the configured set of local crates.
    ///
    /// Returns `false` if no local crates have been configured.
    #[salsa::tracked]
    pub fn is_local(self, db: &'db dyn Db) -> bool {
        LocalCrates::contains(db, self.root(db).file_path(db))
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.lower_name(db)
    }

    pub fn root(self, db: &'db dyn Db) -> Module<'db> {
        self.lower_crate_root(db)
    }

    pub fn dir(self, db: &'db dyn Db) -> &'db FilePath {
        self.lower_dir(db)
    }

    pub fn all_modules(self, db: &'db dyn Db) -> impl Iterator<Item = Module<'db>> + 'db {
        let mut pending = VecDeque::from([self.root(db)]);
        std::iter::from_fn(move || {
            let module = pending.pop_front()?;
            pending.extend(module.modules(db));
            Some(module)
        })
    }
}

#[salsa::interned(debug)]
pub struct Module<'db> {
    hir: HirModule,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ModuleParent<'db> {
    Module(Module<'db>),
    Crate(Crate<'db>),
}

#[salsa::tracked]
impl<'db> Module<'db> {
    pub fn parent(self, db: &'db dyn Db) -> ModuleParent<'db> {
        self.lower_parent(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.lower_name(db)
    }

    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        match self.parent(db) {
            ModuleParent::Module(parent) => parent
                .path(db)
                .append(db, PathSegment::new(db, self.name(db).to_owned())),
            ModuleParent::Crate(crate_) => {
                ItemPath::from_segment(db, crate_, PathSegment::new(db, self.name(db).to_owned()))
            }
        }
    }

    pub fn members(self, db: &'db dyn Db) -> &'db Vec<Member<'db>> {
        self.lower_members(db)
    }

    pub fn modules(self, db: &'db dyn Db) -> impl Iterator<Item = Module<'db>> + 'db {
        self.members(db).iter().filter_map(|member| match member {
            Member::Module(module) => Some(*module),
            _ => None,
        })
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db).as_ref()
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.lower_span(db).as_ref()
    }

    pub fn file_path(self, db: &'db dyn Db) -> &'db FilePath {
        self.lower_file_path(db)
    }
}

as_item!(Module);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum Member<'db> {
    Struct(Struct<'db>),
    Enum(Enum<'db>),
    Function(Function<'db>),
    Module(Module<'db>),
}

#[salsa::interned(debug)]
pub struct Struct<'db> {
    hir: HirStruct,
}

#[salsa::tracked]
impl<'db> Struct<'db> {
    pub fn module(self, db: &'db dyn Db) -> Module<'db> {
        self.lower_parent(db)
    }

    pub fn generic_sig(self, db: &'db dyn Db) -> GenericSig<'db> {
        self.lower_generic_sig(db)
    }

    pub fn segment(self, db: &'db dyn Db) -> PathSegment<'db> {
        PathSegment::new(db, self.lower_name(db).to_owned())
    }

    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.module(db).path(db).append(db, self.segment(db))
    }

    pub fn shape(self, db: &'db dyn Db) -> &'db Shape<'db> {
        self.lower_shape(db)
    }

    pub fn is_non_exhaustive(self, db: &'db dyn Db) -> bool {
        self.lower_is_non_exhaustive(db)
    }

    pub fn is_clonable(self, db: &'db dyn Db) -> bool {
        self.lower_is_clonable(db)
    }

    /// Whether the struct can be constructed using struct expression syntax.
    pub fn is_constructible(self, db: &'db dyn Db) -> bool {
        !self.is_non_exhaustive(db)
            && !self.shape(db).has_stripped_fields()
            && self.is_clonable(db)
            && self.fields(db).all(|field| !field.is_readonly(db))
    }

    /// Flattens the shape while omitting private or hidden tuple-field slots.
    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = Field<'db>> + 'db {
        match self.shape(db) {
            Shape::Unit => Either::Left(std::iter::empty()),
            Shape::Tuple(fields) => Either::Right(Either::Left(fields.iter().copied().flatten())),
            Shape::Plain { fields, .. } => Either::Right(Either::Right(fields.iter().copied())),
        }
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db).as_ref()
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.lower_span(db).as_ref()
    }

    /// Supported inherent impl blocks for this type, including impls declared
    /// in other modules.
    pub fn impls(self, db: &'db dyn Db) -> &'db Vec<Impl<'db>> {
        self.lower_impls(db)
    }
}

impl<'db> IntoValueType<'db> for Struct<'db> {
    fn into_value_type(self) -> ValueType<'db> {
        ValueType::Struct(self)
    }
}

as_item!(Struct);

#[salsa::interned(debug)]
pub struct Enum<'db> {
    hir: HirEnum,
}

#[salsa::tracked]
impl<'db> Enum<'db> {
    pub fn module(self, db: &'db dyn Db) -> Module<'db> {
        self.lower_parent(db)
    }

    pub fn generic_sig(self, db: &'db dyn Db) -> GenericSig<'db> {
        self.lower_generic_sig(db)
    }

    pub fn segment(self, db: &'db dyn Db) -> PathSegment<'db> {
        PathSegment::new(db, self.lower_name(db).to_owned())
    }

    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.module(db).path(db).append(db, self.segment(db))
    }

    pub fn is_non_exhaustive(self, db: &'db dyn Db) -> bool {
        self.lower_is_non_exhaustive(db)
    }

    pub fn is_clonable(self, db: &'db dyn Db) -> bool {
        self.lower_is_clonable(db)
    }

    /// The variants of this enum.
    pub fn variants(self, db: &'db dyn Db) -> &'db Vec<Variant<'db>> {
        self.lower_variants(db)
    }

    /// Concatenates each variant's visible fields in variant order.
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<Field<'db>> {
        let mut fields = vec![];

        for variant in self.variants(db) {
            fields.extend(variant.fields(db));
        }

        fields
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db).as_ref()
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.lower_span(db).as_ref()
    }

    /// Supported inherent impl blocks for this type, including impls declared
    /// in other modules.
    pub fn impls(self, db: &'db dyn Db) -> &'db Vec<Impl<'db>> {
        self.lower_impls(db)
    }
}

impl<'db> IntoValueType<'db> for Enum<'db> {
    fn into_value_type(self) -> ValueType<'db> {
        ValueType::Enum(self)
    }
}

as_item!(Enum);

/// A supported inherent impl block for a struct or enum.
#[salsa::interned(debug)]
pub struct Impl<'db> {
    hir: HirImpl,
}

#[salsa::tracked]
impl<'db> Impl<'db> {
    pub fn module(self, db: &'db dyn Db) -> Module<'db> {
        self.lower_module(db)
    }

    pub fn generic_sig(self, db: &'db dyn Db) -> GenericSig<'db> {
        self.lower_generic_sig(db)
    }

    pub fn target(self, db: &'db dyn Db) -> ImplTarget<'db> {
        self.lower_target(db)
    }

    pub fn functions(self, db: &'db dyn Db) -> &'db Vec<Function<'db>> {
        self.lower_functions(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.lower_span(db).as_ref()
    }
}

/// The nominal type targeted by an inherent impl block.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ImplTarget<'db> {
    Struct(Struct<'db>),
    Enum(Enum<'db>),
}

impl<'db> ImplTarget<'db> {
    pub fn parent(self, db: &'db dyn Db) -> Module<'db> {
        match self {
            ImplTarget::Struct(struct_) => struct_.module(db),
            ImplTarget::Enum(enum_) => enum_.module(db),
        }
    }

    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        match self {
            ImplTarget::Struct(struct_) => struct_.path(db),
            ImplTarget::Enum(enum_) => enum_.path(db),
        }
    }
}

impl<'db> IntoValueType<'db> for ImplTarget<'db> {
    fn into_value_type(self) -> ValueType<'db> {
        match self {
            ImplTarget::Struct(struct_) => struct_.into_value_type(),
            ImplTarget::Enum(enum_) => enum_.into_value_type(),
        }
    }
}

impl<'db> AsItem<'db> for ImplTarget<'db> {
    fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.path(db)
    }
}

#[salsa::interned(debug)]
pub struct Variant<'db> {
    hir: HirVariant,
}

#[salsa::tracked]
impl<'db> Variant<'db> {
    pub fn parent(self, db: &'db dyn Db) -> Enum<'db> {
        self.lower_parent(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.lower_name(db)
    }

    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.parent(db)
            .path(db)
            .append(db, PathSegment::new(db, self.name(db).to_owned()))
    }

    pub fn shape(self, db: &'db dyn Db) -> &'db Shape<'db> {
        self.lower_shape(db)
    }

    pub fn is_non_exhaustive(self, db: &'db dyn Db) -> bool {
        self.lower_is_non_exhaustive(db)
    }

    /// Whether the variant can be constructed using struct expression syntax.
    pub fn is_constructible(self, db: &'db dyn Db) -> bool {
        !self.is_non_exhaustive(db)
            && !self.shape(db).has_stripped_fields()
            && self.parent(db).is_clonable(db)
            && self.fields(db).all(|field| !field.is_readonly(db))
    }

    /// Flattens the shape while omitting private or hidden tuple-field slots.
    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = Field<'db>> + 'db {
        match self.shape(db) {
            Shape::Unit => Either::Left(std::iter::empty()),
            Shape::Tuple(fields) => Either::Right(Either::Left(fields.iter().copied().flatten())),
            Shape::Plain { fields, .. } => Either::Right(Either::Right(fields.iter().copied())),
        }
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db).as_ref()
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.lower_span(db).as_ref()
    }
}

as_item!(Variant);

#[derive(Debug, Clone, Hash, PartialEq, Eq, salsa::Update)]
pub enum Shape<'db> {
    Unit,
    /// Tuple fields, where omitted fields are represented by `None`.
    Tuple(Vec<Option<Field<'db>>>),
    Plain {
        fields: Vec<Field<'db>>,
        /// Whether fields were omitted, including private or unsupported
        /// fields.
        has_stripped_fields: bool,
    },
}

impl Shape<'_> {
    /// Whether lowering omitted any source fields. Such shapes cannot be
    /// constructed using only the exposed fields.
    pub fn has_stripped_fields(&self) -> bool {
        match self {
            Shape::Unit => false,
            Shape::Tuple(fields) => fields.iter().any(Option::is_none),
            Shape::Plain {
                has_stripped_fields,
                ..
            } => *has_stripped_fields,
        }
    }
}

#[salsa::interned(debug)]
pub struct Field<'db> {
    hir: HirField,
}

#[salsa::tracked]
impl<'db> Field<'db> {
    pub fn parent(self, db: &'db dyn Db) -> FieldParent<'db> {
        self.lower_parent(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db FieldName {
        self.lower_name(db)
    }

    /// Field spelling for Rust 2024 expressions and patterns.
    pub fn escaped_name(self, db: &'db dyn Db) -> &'db String {
        self.lower_escaped_name(db)
    }

    pub fn value_type(self, db: &'db dyn Db) -> ValueType<'db> {
        self.lower_value_type(db)
    }

    /// Whether generated bindings must prohibit writes to the field.
    pub fn is_readonly(self, db: &'db dyn Db) -> bool {
        self.lower_is_readonly(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db).as_ref()
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.lower_span(db).as_ref()
    }

    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        let name = match self.name(db) {
            FieldName::Idx(idx) => format!("{}", idx),
            FieldName::Name(name) => name.to_owned(),
        };

        let segment = PathSegment::new(db, name);
        match self.parent(db) {
            FieldParent::Struct(struct_) => struct_.path(db).append(db, segment),
            FieldParent::Variant(variant) => variant.path(db).append(db, segment),
        }
    }

    /// Enum-variant fields are prefixed with their variant to avoid collisions.
    #[salsa::tracked(returns(ref))]
    pub fn mangled_name(self, db: &'db dyn Db) -> String {
        match self.parent(db) {
            FieldParent::Struct(_) => self.name(db).to_string(),
            FieldParent::Variant(variant) => {
                format!("__{}__{}", variant.name(db), self.name(db))
            }
        }
    }
}

as_item!(Field);

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, salsa::Update, salsa::Supertype)]
pub enum FieldParent<'db> {
    Struct(Struct<'db>),
    Variant(Variant<'db>),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, salsa::Update)]
pub enum FieldName {
    Idx(usize),
    Name(String),
}

impl fmt::Display for FieldName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldName::Idx(idx) => write!(f, "{}", idx),
            FieldName::Name(name) => write!(f, "{}", name),
        }
    }
}

/// An item's own normalized generic parameters.
///
/// Enclosing parameters remain on their declaring item. HIR orders lifetimes
/// first, followed by type and const parameters in declaration order.
#[salsa::interned(debug)]
pub struct GenericSig<'db> {
    #[returns(ref)]
    pub params: Vec<GenericParam<'db>>,
}

impl<'db> GenericSig<'db> {
    pub fn is_empty(self, db: &'db dyn Db) -> bool {
        self.params(db).is_empty()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum GenericParam<'db> {
    Type(TypeParam<'db>),
    Const(ConstParam<'db>),
    Lifetime(LifetimeParam<'db>),
}

impl<'db> GenericParam<'db> {
    /// The declared name, or `None` for an implicit parameter.
    pub fn name(self, db: &'db dyn Db) -> Option<&'db str> {
        self.lower_name(db).as_deref()
    }

    pub fn owner(self, db: &'db dyn Db) -> GenericOwner<'db> {
        self.lower_owner(db)
    }
}

/// The declaration that introduces a generic parameter.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum GenericOwner<'db> {
    Function(Function<'db>),
    Struct(Struct<'db>),
    Enum(Enum<'db>),
    Impl(Impl<'db>),
}

#[salsa::interned(debug)]
pub struct TypeParam<'db> {
    hir: HirTypeParam,
}

impl<'db> TypeParam<'db> {
    pub fn name(self, db: &'db dyn Db) -> Option<&'db str> {
        GenericParam::Type(self).name(db)
    }

    pub fn owner(self, db: &'db dyn Db) -> GenericOwner<'db> {
        GenericParam::Type(self).owner(db)
    }

    /// Whether this parameter was introduced by argument-position `impl Trait`.
    pub fn is_implicit(self, db: &'db dyn Db) -> bool {
        self.hir(db).is_implicit(db)
    }
}

#[salsa::interned(debug)]
pub struct ConstParam<'db> {
    hir: HirConstParam,
}

impl<'db> ConstParam<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        GenericParam::Const(self)
            .name(db)
            .expect("const parameters have declared names")
    }

    pub fn owner(self, db: &'db dyn Db) -> GenericOwner<'db> {
        GenericParam::Const(self).owner(db)
    }
}

#[salsa::interned(debug)]
pub struct LifetimeParam<'db> {
    hir: HirLifetimeParam,
}

impl<'db> LifetimeParam<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        GenericParam::Lifetime(self)
            .name(db)
            .expect("lifetime parameters have declared names")
    }

    pub fn owner(self, db: &'db dyn Db) -> GenericOwner<'db> {
        GenericParam::Lifetime(self).owner(db)
    }
}

#[salsa::interned(debug)]
pub struct Function<'db> {
    hir: HirFunction,
}

#[salsa::tracked]
impl<'db> Function<'db> {
    pub fn parent(self, db: &'db dyn Db) -> FunctionParent<'db> {
        self.lower_parent(db)
    }

    pub fn generic_sig(self, db: &'db dyn Db) -> GenericSig<'db> {
        self.lower_generic_sig(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.lower_name(db)
    }

    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.parent(db)
            .path(db)
            .append(db, PathSegment::new(db, self.name(db).to_owned()))
    }

    pub fn receiver(self, db: &'db dyn Db) -> Option<Receiver> {
        self.lower_receiver(db)
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<Param<'db>> {
        self.lower_params(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ValueType<'db>> {
        self.lower_ret_type(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db).as_ref()
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.lower_span(db).as_ref()
    }
}

as_item!(Function);

/// The declaration containing a source-defined function.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum FunctionParent<'db> {
    Module(Module<'db>),
    Impl(Impl<'db>),
}

impl<'db> FunctionParent<'db> {
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        match self {
            FunctionParent::Module(mod_) => mod_.path(db),
            FunctionParent::Impl(impl_) => impl_.target(db).path(db),
        }
    }
}

/// How an associated function receives its target value.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Receiver {
    /// `&self`
    Shared,
    /// `&mut self`
    Exclusive,
    /// `self`
    Owned,
}

#[salsa::tracked(debug)]
pub struct Param<'db> {
    pub parent: Function<'db>,
    pub index: usize,
    #[returns(ref)]
    #[tracked]
    pub name: String,
    #[returns(ref)]
    #[tracked]
    pub type_: Type<'db>,
    #[returns(ref)]
    #[tracked]
    pub span: Option<Span>,
}

/// A supported subset of Rust value types.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ValueType<'db> {
    bool,
    char,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    f32,
    f64,
    Str,
    String,
    Struct(Struct<'db>),
    Enum(Enum<'db>),
}

impl<'db> ValueType<'db> {
    pub fn fqn(self, db: &'db dyn Db) -> &'db str {
        match self {
            ValueType::bool => "bool",
            ValueType::char => "char",
            ValueType::i8 => "i8",
            ValueType::u8 => "u8",
            ValueType::i16 => "i16",
            ValueType::u16 => "u16",
            ValueType::i32 => "i32",
            ValueType::u32 => "u32",
            ValueType::f32 => "f32",
            ValueType::f64 => "f64",
            ValueType::Str => "str",
            ValueType::String => "String",
            ValueType::Struct(s) => s.fqn(db),
            ValueType::Enum(e) => e.fqn(db),
        }
    }

    pub fn borrowed(self) -> Type<'db> {
        Type::Borrowed {
            is_mutable: false,
            inner: self,
        }
    }

    pub fn borrowed_mut(self) -> Type<'db> {
        Type::Borrowed {
            is_mutable: true,
            inner: self,
        }
    }

    pub fn as_type(self) -> Type<'db> {
        Type::Value(self)
    }
}

impl<'db> From<ValueType<'db>> for Type<'db> {
    fn from(value: ValueType<'db>) -> Self {
        Type::Value(value)
    }
}

pub trait IntoValueType<'db> {
    fn into_value_type(self) -> ValueType<'db>;
}

impl<'db, T: IntoValueType<'db>> From<T> for ValueType<'db> {
    fn from(value: T) -> Self {
        value.into_value_type()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Type<'db> {
    Value(ValueType<'db>),
    Borrowed {
        is_mutable: bool,
        inner: ValueType<'db>,
    },
}

impl<'db> Type<'db> {
    pub fn deborrow(self) -> ValueType<'db> {
        match self {
            Type::Value(value_type) => value_type,
            Type::Borrowed { inner, .. } => inner,
        }
    }
}

pub trait IntoType<'db> {
    fn into_type(self) -> Type<'db>;
}

impl<'db, T: IntoValueType<'db>> IntoType<'db> for T {
    fn into_type(self) -> Type<'db> {
        Type::Value(self.into_value_type())
    }
}

impl<'db, T: IntoType<'db>> From<T> for Type<'db> {
    fn from(value: T) -> Self {
        value.into_type()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Span {
    pub src_path: FilePath,
    pub offset: usize,
    pub length: usize,
}

impl Span {
    pub fn src(&self, db: &dyn Db) -> Arc<str> {
        self.src_path.contents(db).unwrap_or_else(|| Arc::from(""))
    }
}
