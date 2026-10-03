//! Intermediate representation for the supported subset of Rust's public API.

mod lowering;
mod mangle;

use crate::compiler::{db::Db, inputs::LocalCrates, path::FilePath};
use itertools::Either;
use ra_ap_hir::{
    Crate as HirCrate, Enum as HirEnum, EnumVariant as HirVariant, Field as HirField,
    Function as HirFunction, Impl as HirImpl, Module as HirModule, Struct as HirStruct,
    TypeParam as HirTypeParam,
};
use std::{collections::VecDeque, fmt};
use triomphe::Arc;

pub mod usages;
pub use mangle::MangleError;

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
    #[returns(ref)]
    pub gen_args: Vec<GenArg<'db>>,
}

#[salsa::tracked]
impl<'db> PathSegment<'db> {
    pub fn plain(db: &'db dyn Db, name: String) -> Self {
        Self::new(db, name, Vec::<GenArg<'db>>::new())
    }

    #[salsa::tracked(returns(ref))]
    pub fn display(self, db: &'db dyn Db) -> String {
        if self.gen_args(db).is_empty() {
            return self.name(db).to_owned();
        }
        let args = self
            .gen_args(db)
            .iter()
            .map(|arg| arg.display(db))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}::<{args}>", self.name(db))
    }

    /// Mangling is fallible for arguments without a stable closed identity.
    #[salsa::tracked(returns(ref))]
    pub fn try_mangled(self, db: &'db dyn Db) -> Result<String, MangleError> {
        mangle::mangle_segments(db, &[self])
    }

    /// For code generation, which only receives supported, closed paths.
    pub fn mangled(self, db: &'db dyn Db) -> &'db String {
        self.try_mangled(db)
            .as_ref()
            .expect("cannot mangle a path with unresolved generic arguments")
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
            .map(|segment| segment.display(db).as_str())
            .collect::<Vec<_>>()
            .join("::")
    }

    /// Mangling is fallible for arguments without a stable closed identity.
    //
    // FIXME: add a `ItemMonoPath` that is a variant of `ItemPath` that does not
    // include generic params and add the mangled method on this one?
    #[salsa::tracked(returns(ref))]
    pub fn try_mangled(self, db: &'db dyn Db) -> Result<String, MangleError> {
        mangle::mangle_segments(db, self.segments(db))
    }

    /// For code generation, which only receives supported, closed paths.
    pub fn mangled(self, db: &'db dyn Db) -> &'db String {
        self.try_mangled(db)
            .as_ref()
            .expect("cannot mangle a path with unresolved generic arguments")
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
                .append(db, PathSegment::plain(db, self.name(db).to_owned())),
            ModuleParent::Crate(crate_) => {
                ItemPath::from_segment(db, crate_, PathSegment::plain(db, self.name(db).to_owned()))
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
    Function(FunctionFree<'db>),
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

    pub fn generic_params(self, db: &'db dyn Db) -> &'db Vec<GenericParam<'db>> {
        self.lower_generic_params(db)
    }

    pub fn segment(self, db: &'db dyn Db) -> PathSegment<'db> {
        PathSegment::plain(db, self.lower_name(db).to_owned())
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

    /// The struct's type as a [`Type`] if it is a monomorphized struct (i.e.
    /// without generics).
    pub fn as_mono_type(self, db: &'db dyn Db) -> Option<Type<'db>> {
        self.lower_as_mono_type(db)
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

    pub fn generic_params(self, db: &'db dyn Db) -> &'db Vec<GenericParam<'db>> {
        self.lower_generic_params(db)
    }

    pub fn segment(self, db: &'db dyn Db) -> PathSegment<'db> {
        PathSegment::plain(db, self.lower_name(db).to_owned())
    }

    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.module(db).path(db).append(db, self.segment(db))
    }

    pub fn is_non_exhaustive(self, db: &'db dyn Db) -> bool {
        self.lower_is_non_exhaustive(db)
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

    /// The enum's type as a [`Type`] if it is a monomorphized enum (i.e.
    /// without generics).
    pub fn as_mono_type(self, db: &'db dyn Db) -> Option<Type<'db>> {
        self.lower_as_mono_type(db)
    }
}

as_item!(Enum);

/// A supported inherent impl block.
#[salsa::interned(debug)]
pub struct Impl<'db> {
    hir: HirImpl,
}

#[salsa::tracked]
impl<'db> Impl<'db> {
    pub fn module(self, db: &'db dyn Db) -> Module<'db> {
        self.lower_module(db)
    }

    pub fn generic_params(self, db: &'db dyn Db) -> &'db Vec<GenericParam<'db>> {
        self.lower_generic_params(db)
    }

    pub fn functions(self, db: &'db dyn Db) -> &'db Vec<Function<'db>> {
        self.lower_functions(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.lower_span(db).as_ref()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ImplQualified<'db> {
    Inherent(ImplInherent<'db>),
}

#[salsa::tracked]
impl<'db> ImplQualified<'db> {
    pub fn declaration(self, db: &'db dyn Db) -> Impl<'db> {
        match self {
            Self::Inherent(impl_) => impl_.declaration(db),
        }
    }

    pub fn functions(self, db: &'db dyn Db) -> impl Iterator<Item = FunctionQualified<'db>> + 'db {
        match self {
            Self::Inherent(impl_) => impl_.functions(db).map(FunctionQualified::Inherent),
        }
    }
}

#[salsa::interned(debug)]
pub struct ImplInherent<'db> {
    pub declaration: Impl<'db>,
    pub target: Type<'db>,
}

#[salsa::tracked]
impl<'db> ImplInherent<'db> {
    pub fn generic_args(self, db: &'db dyn Db) -> &'db Vec<(GenericParam<'db>, GenArg<'db>)> {
        self.lower_generic_args(db)
    }

    pub fn functions(self, db: &'db dyn Db) -> impl Iterator<Item = FunctionInherent<'db>> + 'db {
        self.lower_functions(db).iter().copied()
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
            .append(db, PathSegment::plain(db, self.name(db).to_owned()))
    }

    pub fn shape(self, db: &'db dyn Db) -> &'db Shape<'db> {
        self.lower_shape(db)
    }

    pub fn is_non_exhaustive(self, db: &'db dyn Db) -> bool {
        self.lower_is_non_exhaustive(db)
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

#[salsa::interned(debug)]
pub struct VariantQualified<'db> {
    pub declaration: Variant<'db>,
    pub target: EnumType<'db>,
}

impl<'db> VariantQualified<'db> {
    pub fn has_stripped_fields(self, db: &'db dyn Db) -> bool {
        self.lower_has_stripped_fields(db)
    }

    /// Whether this variant can be constructed from its exposed fields.
    pub fn is_constructible(self, db: &'db dyn Db) -> bool {
        !self.declaration(db).is_non_exhaustive(db)
            && !self.has_stripped_fields(db)
            && self.target(db).is_clonable(db) == Some(true)
            && self.fields(db).all(|field| !field.is_readonly(db))
    }

    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.target(db).path(db).append(
            db,
            PathSegment::plain(db, self.declaration(db).name(db).clone()),
        )
    }

    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = FieldQualified<'db>> + 'db {
        self.lower_fields(db)
    }
}

as_item!(VariantQualified);

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

    pub fn _type(self, db: &'db dyn Db) -> Type<'db> {
        self.lower_type(db)
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

        let segment = PathSegment::plain(db, name);
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

#[salsa::interned(debug)]
pub struct FieldQualified<'db> {
    pub declaration: Field<'db>,
    pub target: Type<'db>,
}

impl<'db> FieldQualified<'db> {
    pub fn _type(self, db: &'db dyn Db) -> Type<'db> {
        Type::new(db, TypeOrigin::QualifiedField(self)).deborrow(db)
    }

    pub fn is_readonly(self, db: &'db dyn Db) -> bool {
        self.lower_is_readonly(db)
    }

    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        let path = match self.target(db).kind(db) {
            TypeKind::Struct(type_) => type_.path(db),
            TypeKind::Enum(type_) => {
                let FieldParent::Variant(variant) = self.declaration(db).parent(db) else {
                    unreachable!("enum fields belong to variants")
                };
                VariantQualified::new(db, variant, type_).path(db)
            }
            _ => unreachable!("qualified fields belong to structs or enums"),
        };
        path.append(
            db,
            PathSegment::plain(db, self.declaration(db).name(db).to_string()),
        )
    }
}

as_item!(FieldQualified);

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum GenArg<'db> {
    Type(Type<'db>),
}

impl<'db> GenArg<'db> {
    pub fn display(self, db: &'db dyn Db) -> &'db str {
        match self {
            Self::Type(type_) => type_.display(db),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum GenericParam<'db> {
    Type(TypeParam<'db>),
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

    pub fn display(self, db: &'db dyn Db) -> &'db str {
        self.lower_display(db)
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
pub struct Function<'db> {
    hir: HirFunction,
}

#[salsa::tracked]
impl<'db> Function<'db> {
    pub fn parent(self, db: &'db dyn Db) -> FunctionParent<'db> {
        self.lower_parent(db)
    }

    pub fn generic_params(self, db: &'db dyn Db) -> &'db Vec<GenericParam<'db>> {
        self.lower_generic_params(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.lower_name(db)
    }

    pub fn segment(self, db: &'db dyn Db) -> PathSegment<'db> {
        PathSegment::plain(db, self.name(db).to_owned())
    }

    pub fn receiver(self, db: &'db dyn Db) -> Option<Receiver> {
        self.lower_receiver(db)
    }

    pub fn receiver_type(self, db: &'db dyn Db) -> Option<Type<'db>> {
        self.receiver(db)
            .map(|_| Type::new(db, TypeOrigin::FunctionReceiver(self)))
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<Param<'db>> {
        self.lower_params(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<Type<'db>> {
        self.lower_ret_type(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db).as_ref()
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.lower_span(db).as_ref()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum FunctionQualified<'db> {
    Free(FunctionFree<'db>),
    Inherent(FunctionInherent<'db>),
}

#[salsa::tracked]
impl<'db> FunctionQualified<'db> {
    pub fn declaration(self, db: &'db dyn Db) -> Function<'db> {
        match self {
            Self::Free(function) => function.declaration(db),
            Self::Inherent(function) => function.declaration(db),
        }
    }

    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        match self {
            Self::Free(function) => function.path(db),
            Self::Inherent(function) => function.path(db),
        }
    }

    pub fn as_callible(self, db: &'db dyn Db) -> Option<Callable<'db>> {
        match self {
            Self::Free(function) => function.as_callible(db),
            Self::Inherent(function) => function.as_callable(db),
        }
    }
}

as_item!(FunctionQualified);

#[salsa::interned(debug)]
pub struct FunctionFree<'db> {
    pub declaration: Function<'db>,
}

impl<'db> FunctionFree<'db> {
    pub fn as_callible(self, db: &'db dyn Db) -> Option<Callable<'db>> {
        self.declaration(db)
            .generic_params(db)
            .is_empty()
            .then(|| Callable::new(db, self.as_qualified(), Vec::<GenArg<'db>>::new()))
    }

    pub fn as_qualified(self) -> FunctionQualified<'db> {
        FunctionQualified::Free(self)
    }

    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        let FunctionParent::Module(module) = self.declaration(db).parent(db) else {
            unreachable!("free functions must be declared in a module")
        };
        module.path(db).append(db, self.declaration(db).segment(db))
    }
}

#[salsa::interned(debug)]
pub struct FunctionInherent<'db> {
    pub declaration: Function<'db>,
    pub impl_: ImplInherent<'db>,
}

impl<'db> FunctionInherent<'db> {
    pub fn as_callable(self, db: &'db dyn Db) -> Option<Callable<'db>> {
        self.lower_as_callable(db)
    }

    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.lower_path(db)
    }
}

#[salsa::interned(debug)]
pub struct Callable<'db> {
    pub function: FunctionQualified<'db>,
    /// Arguments for the function's own generic parameters.
    /// FIXME: currently unused but should be in nearest future
    #[returns(ref)]
    pub generic_args: Vec<GenArg<'db>>,
}

#[salsa::tracked]
impl<'db> Callable<'db> {
    pub fn declaration(self, db: &'db dyn Db) -> Function<'db> {
        self.function(db).declaration(db)
    }

    pub fn segment(self, db: &'db dyn Db) -> PathSegment<'db> {
        PathSegment::new(db, self.declaration(db).name(db), self.generic_args(db))
    }

    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        let path = self.function(db).path(db);
        let mut segments = path.segments(db).clone();
        *segments.last_mut().expect("functions have a path segment") = self.segment(db);
        ItemPath::new(db, path.crate_(db), segments)
    }

    pub fn receiver_type(self, db: &'db dyn Db) -> Option<Type<'db>> {
        if !self.needs_substitution(db) {
            return self.declaration(db).receiver_type(db);
        }
        self.declaration(db)
            .receiver(db)
            .map(|_| Type::new(db, TypeOrigin::CallableReceiver(self)))
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<Param<'db>> {
        self.lower_params(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<Type<'db>> {
        self.lower_ret_type(db)
    }
}

as_item!(Callable);

/// The declaration containing a source-defined function.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum FunctionParent<'db> {
    Module(Module<'db>),
    Impl(Impl<'db>),
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
    pub index: usize,
    #[returns(ref)]
    pub name: String,
    #[returns(ref)]
    pub type_: Type<'db>,
    #[returns(ref)]
    #[tracked]
    pub span: Option<Span>,
}

/// A supported subset of Rust types.
#[salsa::interned(debug)]
pub struct Type<'db> {
    pub origin: TypeOrigin<'db>,
}

/// An origin of rust type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TypeOrigin<'db> {
    Field(Field<'db>),
    QualifiedField(FieldQualified<'db>),
    /// Index into `params_without_self`, not the full parameter list.
    FunctionParameter {
        function: Function<'db>,
        index: usize,
    },
    FunctionReturn(Function<'db>),
    FunctionReceiver(Function<'db>),
    CallableParameter {
        callable: Callable<'db>,
        index: usize,
    },
    CallableReturn(Callable<'db>),
    CallableReceiver(Callable<'db>),
    TypeArgument {
        type_: Type<'db>,
        index: usize,
    },
    Struct(StructType<'db>),
    Enum(EnumType<'db>),
    TypeParameter(TypeParam<'db>),
    Borrowed(BorrowedType<'db>),
    Dereferenced(Type<'db>),
}

impl<'db> TypeOrigin<'db> {
    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        match self {
            TypeOrigin::Field(field) => field.span(db),
            TypeOrigin::QualifiedField(field) => field.declaration(db).span(db),
            TypeOrigin::FunctionParameter { function, index } => {
                function.params(db)[index].span(db).as_ref()
            }
            TypeOrigin::FunctionReturn(function) | TypeOrigin::FunctionReceiver(function) => {
                function.span(db)
            }
            TypeOrigin::CallableParameter { callable, index } => {
                callable.declaration(db).params(db)[index].span(db).as_ref()
            }
            TypeOrigin::CallableReturn(callable) | TypeOrigin::CallableReceiver(callable) => {
                callable.declaration(db).span(db)
            }
            TypeOrigin::Struct(type_) => type_.def(db).span(db),
            TypeOrigin::Enum(type_) => type_.def(db).span(db),
            TypeOrigin::Borrowed(reference) => reference.inner(db).origin(db).span(db),
            TypeOrigin::Dereferenced(type_) | TypeOrigin::TypeArgument { type_, .. } => {
                type_.origin(db).span(db)
            }
            TypeOrigin::TypeParameter(_) => None,
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TypeKind<'db> {
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
    Generic(TypeParam<'db>),
    Struct(StructType<'db>),
    Enum(EnumType<'db>),
    Borrowed(BorrowedType<'db>),
}

impl<'db> Type<'db> {
    pub fn kind(self, db: &'db dyn Db) -> TypeKind<'db> {
        self.lower_kind(db)
    }

    pub fn is_clonable(self, db: &'db dyn Db) -> Option<bool> {
        self.lower_is_clonable(db)
    }

    pub fn impls(self, db: &'db dyn Db) -> &'db Vec<ImplQualified<'db>> {
        self.lower_impls(db)
    }

    // FIXME: we may want to have a .fqn() method or .decl_syntax() method. Both of
    // them feels wrong right now. It might be reasonable to have a separate type
    // like `MonoType` that would represent a type without generic params and add
    // .decl_syntax() on this one.
    pub fn display(self, db: &'db dyn Db) -> &'db str {
        match self.kind(db) {
            TypeKind::bool => "bool",
            TypeKind::char => "char",
            TypeKind::i8 => "i8",
            TypeKind::u8 => "u8",
            TypeKind::i16 => "i16",
            TypeKind::u16 => "u16",
            TypeKind::i32 => "i32",
            TypeKind::u32 => "u32",
            TypeKind::f32 => "f32",
            TypeKind::f64 => "f64",
            TypeKind::Str => "str",
            TypeKind::String => "String",
            TypeKind::Generic(param) => param.display(db),
            TypeKind::Struct(s) => s.fqn(db),
            TypeKind::Enum(e) => e.fqn(db),
            TypeKind::Borrowed(reference) => reference.display(db),
        }
    }

    pub fn borrowed(self, db: &'db dyn Db) -> Self {
        Type::new(db, TypeOrigin::Borrowed(BorrowedType::new(db, false, self)))
    }

    pub fn borrowed_mut(self, db: &'db dyn Db) -> Self {
        Type::new(db, TypeOrigin::Borrowed(BorrowedType::new(db, true, self)))
    }

    /// Removes one reference layer, leaving non-reference types unchanged.
    pub fn deborrow(self, db: &'db dyn Db) -> Self {
        match self.kind(db) {
            TypeKind::Borrowed(reference) => reference.inner(db),
            _ => self,
        }
    }

    fn hir(self, db: &'db dyn Db) -> ra_ap_hir::Type<'db> {
        self.origin(db).hir(db)
    }
}

/// A struct definition applied to a particular list of generic arguments.
#[salsa::interned(debug)]
pub struct StructType<'db> {
    pub def: Struct<'db>,
    #[returns(ref)]
    pub gen_args: Vec<GenArg<'db>>,
}

#[salsa::tracked]
impl<'db> StructType<'db> {
    pub fn into_type(self, db: &'db dyn Db) -> Type<'db> {
        Type::new(db, TypeOrigin::from(self))
    }

    pub fn is_clonable(self, db: &'db dyn Db) -> Option<bool> {
        self.into_type(db).is_clonable(db)
    }

    pub fn has_stripped_fields(self, db: &'db dyn Db) -> bool {
        self.lower_has_stripped_fields(db)
    }

    /// Whether this struct can be constructed from its exposed fields.
    pub fn is_constructible(self, db: &'db dyn Db) -> bool {
        !self.def(db).is_non_exhaustive(db)
            && !self.has_stripped_fields(db)
            && self.is_clonable(db) == Some(true)
            && self.fields(db).all(|field| !field.is_readonly(db))
    }

    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = FieldQualified<'db>> + 'db {
        self.lower_fields(db)
    }

    #[salsa::tracked]
    pub fn segment(self, db: &'db dyn Db) -> PathSegment<'db> {
        PathSegment::new(
            db,
            self.def(db).lower_name(db).to_owned(),
            self.gen_args(db).to_owned(),
        )
    }

    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.def(db)
            .module(db)
            .path(db)
            .append(db, self.segment(db))
    }
}

impl<'db> From<StructType<'db>> for TypeOrigin<'db> {
    fn from(value: StructType<'db>) -> Self {
        Self::Struct(value)
    }
}

as_item!(StructType);

/// An enum definition applied to a particular list of generic arguments.
#[salsa::interned(debug)]
pub struct EnumType<'db> {
    pub def: Enum<'db>,
    #[returns(ref)]
    pub gen_args: Vec<GenArg<'db>>,
}

#[salsa::tracked]
impl<'db> EnumType<'db> {
    pub fn into_type(self, db: &'db dyn Db) -> Type<'db> {
        Type::new(db, TypeOrigin::from(self))
    }

    pub fn is_clonable(self, db: &'db dyn Db) -> Option<bool> {
        self.into_type(db).is_clonable(db)
    }

    pub fn variants(self, db: &'db dyn Db) -> impl Iterator<Item = VariantQualified<'db>> + 'db {
        self.def(db)
            .variants(db)
            .iter()
            .map(move |variant| VariantQualified::new(db, *variant, self))
    }

    #[salsa::tracked]
    pub fn segment(self, db: &'db dyn Db) -> PathSegment<'db> {
        PathSegment::new(
            db,
            self.def(db).lower_name(db).to_owned(),
            self.gen_args(db).to_owned(),
        )
    }

    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.def(db)
            .module(db)
            .path(db)
            .append(db, self.segment(db))
    }
}

impl<'db> From<EnumType<'db>> for TypeOrigin<'db> {
    fn from(value: EnumType<'db>) -> Self {
        Self::Enum(value)
    }
}

as_item!(EnumType);

impl<'db> From<TypeParam<'db>> for TypeOrigin<'db> {
    fn from(value: TypeParam<'db>) -> Self {
        Self::TypeParameter(value)
    }
}

#[salsa::interned(debug)]
pub struct BorrowedType<'db> {
    pub is_mutable: bool,
    pub inner: Type<'db>,
}

#[salsa::tracked]
impl<'db> BorrowedType<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn display(self, db: &'db dyn Db) -> String {
        let mutability = if self.is_mutable(db) { "mut " } else { "" };
        format!("&{mutability}{}", self.inner(db).display(db))
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
