//! The main intermediate representation used by rustier.
//!
//! Interface IR is the bridge between the supported Rust source API and the
//! generated JavaScript-facing API. It is used directly to generate the Rust
//! runtime code and is also the source representation lowered into the facade
//! and FFI IRs.
//!
//! Unlike the downstream IRs, interface entities deliberately keep references
//! to the higher-level Rust IR public.

pub mod ext;
mod lowering;

use std::collections::VecDeque;

use itertools::Either;

use crate::compiler::{
    db::Db,
    ir::rust::{
        Callable, Crate, Enum, EnumType, FieldName, FieldQualified, FunctionQualified, GenArg,
        Module, ModuleParent, Rust, Struct, StructType, Type, TypeKind, VariantQualified,
        usages::UsageGraph,
    },
};

/// A root of the interface IR.
///
/// It's generally redundant API, generally serving as a quality-of-life wrapper
/// for tests.
#[salsa::interned(debug)]
pub struct Interface<'db> {
    pub rust: Rust<'db>,
    pub usages: UsageGraph<'db>,
}

impl<'db> Interface<'db> {
    pub fn crates(self, db: &'db dyn Db) -> impl Iterator<Item = InterCrate<'db>> {
        self.rust(db)
            .crates(db)
            .iter()
            .map(move |crate_| crate_.interface_reachable_from(db, self.usages(db)))
    }
}

/// An interface crate, containing only reachable modules from the
/// [`UsageGraph`].
#[salsa::interned(debug)]
pub struct InterCrate<'db> {
    pub src: Crate<'db>,
    pub usages: UsageGraph<'db>,
}

impl<'db> InterCrate<'db> {
    /// Whether this crate belongs to the configured set of local crates.
    pub fn is_local(self, db: &'db dyn Db) -> bool {
        self.src(db).is_local(db)
    }

    /// The root module of the crate.
    pub fn root(self, db: &'db dyn Db) -> InterModule<'db> {
        self.src(db)
            .root(db)
            .interface_reachable_from(db, self.usages(db))
    }

    /// The root module and all reachable descendants, in breadth-first order.
    pub fn all_modules(self, db: &'db dyn Db) -> impl Iterator<Item = InterModule<'db>> + 'db {
        let mut pending = VecDeque::from([self.root(db)]);
        std::iter::from_fn(move || {
            let module = pending.pop_front()?;
            pending.extend(module.modules(db));
            Some(module)
        })
    }
}

/// An interface crate used as a parent of the module.
///
/// The parent crate deliberately does not provide root module, they should be
/// obtained by converting it into a reachable [`InterCrate`].
#[salsa::interned(debug)]
pub struct InterParentCrate<'db> {
    pub src: Crate<'db>,
}

impl<'db> InterParentCrate<'db> {
    pub fn into_reachable(self, db: &'db dyn Db, usages: UsageGraph<'db>) -> InterCrate<'db> {
        InterCrate::new(db, self.src(db), usages)
    }
}

/// An interface module used as a parent of other interface entity.
///
/// The parent module deliberately does not provide members, they should be
/// obtained by converting it into a reachable [`InterModule`].
#[salsa::interned(debug)]
pub struct InterParentModule<'db> {
    pub src: Module<'db>,
}

#[salsa::tracked]
impl<'db> InterParentModule<'db> {
    pub fn parent(self, db: &'db dyn Db) -> InterParent<'db> {
        match self.src(db).parent(db) {
            ModuleParent::Module(module) => InterParent::Module(module.interface_parent(db)),
            ModuleParent::Crate(crate_) => InterParent::Crate(crate_.interface_parent(db)),
        }
    }

    pub fn into_reachable(self, db: &'db dyn Db, usages: UsageGraph<'db>) -> InterModule<'db> {
        InterModule::new(db, self.src(db), usages)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum InterParent<'db> {
    Module(InterParentModule<'db>),
    Crate(InterParentCrate<'db>),
}

/// An interface module derived from a rust module, containing only reachable
/// members from the [`UsageGraph`].
#[salsa::interned(debug)]
pub struct InterModule<'db> {
    pub src: Module<'db>,
    pub usages: UsageGraph<'db>,
}

#[salsa::tracked]
impl<'db> InterModule<'db> {
    pub fn as_parent(self, db: &'db dyn Db) -> InterParentModule<'db> {
        self.src(db).interface_parent(db)
    }

    pub fn members(self, db: &'db dyn Db) -> impl Iterator<Item = InterMember<'db>> + 'db {
        self.lower_members(db).iter().cloned()
    }

    /// All reachable interface classes defined in the module.
    pub fn classes(self, db: &'db dyn Db) -> impl Iterator<Item = InterClass<'db>> + 'db {
        self.members(db).flat_map(move |member| match member {
            InterMember::Class(class) => Either::Left(Some(class).into_iter()),
            InterMember::Template(template) => Either::Right(template.classes(db)),
            _ => Either::Left(None.into_iter()),
        })
    }

    /// All reachable interface class templates defined in the module.
    pub fn templates(self, db: &'db dyn Db) -> impl Iterator<Item = InterClassTemplate<'db>> + 'db {
        self.members(db).filter_map(|member| match member {
            InterMember::Template(template) => Some(template),
            _ => None,
        })
    }

    /// All reachable free functions defined in the module.
    pub fn functions(self, db: &'db dyn Db) -> impl Iterator<Item = InterFunctionFree<'db>> + 'db {
        self.members(db).filter_map(|member| match member {
            InterMember::Function(function) => Some(function),
            _ => None,
        })
    }

    /// All reachable child modules defined in the module.
    pub fn modules(self, db: &'db dyn Db) -> impl Iterator<Item = InterModule<'db>> + 'db {
        self.members(db).filter_map(|member| match member {
            InterMember::Module(module) => Some(module),
            _ => None,
        })
    }
}

/// A member reachable from a [`InterModule`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum InterMember<'db> {
    Class(InterClass<'db>),
    Template(InterClassTemplate<'db>),
    Function(InterFunctionFree<'db>),
    Module(InterModule<'db>),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum InterClassTemplate<'db> {
    Plain(InterClassPlainTemplate<'db>),
    Discriminated(InterClassDiscriminatedTemplate<'db>),
}

#[salsa::tracked]
impl<'db> InterClassTemplate<'db> {
    pub fn module(self, db: &'db dyn Db) -> InterParentModule<'db> {
        match self {
            Self::Plain(template) => template.src(db).module(db).interface_parent(db),
            Self::Discriminated(template) => template.src(db).module(db).interface_parent(db),
        }
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self {
            Self::Plain(template) => template.src(db).segment(db).name(db),
            Self::Discriminated(template) => template.src(db).segment(db).name(db),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn param_names(self, db: &'db dyn Db) -> Vec<String> {
        let params = match self {
            Self::Plain(template) => template.src(db).generic_params(db),
            Self::Discriminated(template) => template.src(db).generic_params(db),
        };

        params
            .iter()
            .map(|param| {
                param
                    .name(db)
                    .expect("ADT type parameters are named")
                    .to_owned()
            })
            .collect()
    }

    pub fn classes(self, db: &'db dyn Db) -> impl Iterator<Item = InterClass<'db>> + 'db {
        match self {
            Self::Plain(template) => Either::Left(template.classes(db).map(InterClass::Plain)),
            Self::Discriminated(template) => {
                Either::Right(template.classes(db).map(InterClass::Discriminated))
            }
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn mappings(self, db: &'db dyn Db) -> Vec<(Vec<InterGeneric<'db>>, InterClass<'db>)> {
        let mut entries: Vec<_> = self
            .classes(db)
            .map(|class| (class.generics(db).collect(), class))
            .collect();

        entries.sort_by(|(_, left), (_, right)| left.name(db).cmp(right.name(db)));

        entries
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InterGeneric<'db> {
    Symbol(String),
    Class(InterClass<'db>),
}

#[salsa::interned(debug)]
pub struct InterClassPlainTemplate<'db> {
    pub src: Struct<'db>,
    pub usages: UsageGraph<'db>,
}

impl<'db> InterClassPlainTemplate<'db> {
    pub fn classes(self, db: &'db dyn Db) -> impl Iterator<Item = InterClassPlain<'db>> + 'db {
        let src = self.src(db);

        self.usages(db)
            .types(db)
            .filter_map(move |type_| match type_.kind(db) {
                TypeKind::Struct(type_) if type_.def(db) == src => Some(type_.interface(db)),
                _ => None,
            })
    }
}

#[salsa::interned(debug)]
pub struct InterClassDiscriminatedTemplate<'db> {
    pub src: Enum<'db>,
    pub usages: UsageGraph<'db>,
}

impl<'db> InterClassDiscriminatedTemplate<'db> {
    pub fn classes(
        self,
        db: &'db dyn Db,
    ) -> impl Iterator<Item = InterClassDiscriminated<'db>> + 'db {
        let src = self.src(db);

        self.usages(db)
            .types(db)
            .filter_map(move |type_| match type_.kind(db) {
                TypeKind::Enum(type_) if type_.def(db) == src => Some(type_.interface(db)),
                _ => None,
            })
    }
}

/// A rust ADT represented as a class which is a more friendly representation
/// for a js.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum InterClass<'db> {
    Plain(InterClassPlain<'db>),
    Discriminated(InterClassDiscriminated<'db>),
}

impl<'db> InterClass<'db> {
    /// The module this class belongs to.
    pub fn module(self, db: &'db dyn Db) -> InterParentModule<'db> {
        match self {
            InterClass::Plain(p) => p.module(db),
            InterClass::Discriminated(d) => d.module(db),
        }
    }

    /// The name of this class.
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self {
            InterClass::Plain(p) => p.name(db),
            InterClass::Discriminated(d) => d.name(db),
        }
    }

    /// The symbol of this class.
    pub fn symbol(self, db: &'db dyn Db) -> &'db String {
        match self {
            InterClass::Plain(p) => p.symbol(db),
            InterClass::Discriminated(d) => d.symbol(db),
        }
    }

    /// The source type of this class.
    pub fn src_type(self, db: &'db dyn Db) -> Type<'db> {
        match self {
            InterClass::Plain(p) => p.src(db).into_type(db),
            InterClass::Discriminated(d) => d.src(db).into_type(db),
        }
    }

    pub fn generics(self, db: &'db dyn Db) -> impl Iterator<Item = InterGeneric<'db>> + 'db {
        let args = match self {
            Self::Plain(class) => class.src(db).gen_args(db),
            Self::Discriminated(class) => class.src(db).gen_args(db),
        };

        args.iter()
            .map(move |GenArg::Type(type_)| match type_.interface_out(db) {
                InterType::Class { inner, .. } => InterGeneric::Class(inner),
                _ => InterGeneric::Symbol(type_.display(db).to_owned()),
            })
    }

    /// Fields of this class, mapped from the source Rust ADT.
    pub fn fields(self, db: &'db dyn Db) -> &'db Vec<InterField<'db>> {
        match self {
            InterClass::Plain(p) => p.fields(db),
            InterClass::Discriminated(d) => d.fields(db),
        }
    }

    /// Methods of this class.
    pub fn methods(self, db: &'db dyn Db) -> &'db Vec<InterFunctionAssociated<'db>> {
        match self {
            InterClass::Plain(p) => p.methods(db),
            InterClass::Discriminated(d) => d.methods(db),
        }
    }

    /// All functions of this class, including associated functions,
    /// getters/setters and other synthetic functions.
    pub fn all_functions(self, db: &'db dyn Db) -> &'db Vec<InterFunction<'db>> {
        match self {
            InterClass::Plain(p) => p.all_functions(db),
            InterClass::Discriminated(d) => d.all_functions(db),
        }
    }
}

/// A POD-like class.
#[salsa::interned(debug)]
pub struct InterClassPlain<'db> {
    /// FIXME: derives identity from the struct type which deends on the
    /// "unstable" Type
    pub src: StructType<'db>,
}

#[salsa::tracked]
impl<'db> InterClassPlain<'db> {
    /// The module this class belongs to.
    pub fn module(self, db: &'db dyn Db) -> InterParentModule<'db> {
        self.src(db).def(db).module(db).interface_parent(db)
    }

    /// The name of this class.
    #[salsa::tracked(returns(ref))]
    pub fn name(self, db: &'db dyn Db) -> String {
        // FIXME: remove
        let segment = self.src(db).segment(db);
        if segment.gen_args(db).is_empty() {
            segment.name(db).clone()
        } else {
            format!("_{}", segment.mangled(db))
        }
    }

    /// The symbol of this class.
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__plain__{}", self.src(db).path(db).mangled(db))
    }

    /// Fields of this class.
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<InterField<'db>> {
        self.src(db)
            .fields(db)
            .map(|field| field.interface(db))
            .collect()
    }

    /// Methods of this class.
    #[salsa::tracked(returns(ref))]
    pub fn methods(self, db: &'db dyn Db) -> Vec<InterFunctionAssociated<'db>> {
        self.src(db)
            .into_type(db)
            .impls(db)
            .iter()
            .flat_map(|impl_| impl_.functions(db))
            .filter_map(|function| function.as_callible(db))
            .map(|callable| InterFunctionAssociated::new(db, callable))
            .collect()
    }

    pub fn coerce(self, db: &'db dyn Db) -> Option<InterFunctionPlainCoerce<'db>> {
        self.src(db)
            .is_constructible(db)
            .then(|| InterFunctionPlainCoerce::new(db, self))
    }

    /// All functions defined by this class, including field accessors and
    /// methods.
    #[salsa::tracked(returns(ref))]
    pub fn all_functions(self, db: &'db dyn Db) -> Vec<InterFunction<'db>> {
        self.methods(db)
            .iter()
            .copied()
            .map(|function| function.into())
            .chain(self.fields(db).iter().map(|f| f.getter(db).into()))
            .chain(
                self.fields(db)
                    .iter()
                    .filter_map(|f| f.setter(db).map(|f| f.into())),
            )
            .chain(self.coerce(db).map(|f| f.into()).into_iter())
            .collect()
    }
}

impl<'db> From<InterClassPlain<'db>> for InterClass<'db> {
    fn from(value: InterClassPlain<'db>) -> Self {
        InterClass::Plain(value)
    }
}

/// A tagged discriminated union class.
#[salsa::interned(debug)]
pub struct InterClassDiscriminated<'db> {
    /// FIXME: derives identity from the struct type which deends on the
    /// "unstable" Type
    pub src: EnumType<'db>,
}

#[salsa::tracked]
impl<'db> InterClassDiscriminated<'db> {
    /// The module this class belongs to.
    pub fn module(self, db: &'db dyn Db) -> InterParentModule<'db> {
        self.src(db).def(db).module(db).interface_parent(db)
    }

    /// The name of this class.
    #[salsa::tracked(returns(ref))]
    pub fn name(self, db: &'db dyn Db) -> String {
        // FIXME: remove
        let segment = self.src(db).segment(db);
        if segment.gen_args(db).is_empty() {
            segment.name(db).clone()
        } else {
            format!("_{}", segment.mangled(db))
        }
    }

    /// The mangled unique symbol name of this class.
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__discr__{}", self.src(db).path(db).mangled(db))
    }

    /// The discriminated variants of this class.
    #[salsa::tracked(returns(ref))]
    pub fn variants(self, db: &'db dyn Db) -> Vec<InterClassDiscriminatedVariant<'db>> {
        self.src(db)
            .variants(db)
            .enumerate()
            .map(|(i, variant)| InterClassDiscriminatedVariant::new(db, self, variant, i as u32))
            .collect()
    }

    /// All fields of this class combined
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<InterField<'db>> {
        self.variants(db)
            .iter()
            .flat_map(|v| v.fields(db))
            .collect()
    }

    /// Methods of this class.
    #[salsa::tracked(returns(ref))]
    pub fn methods(self, db: &'db dyn Db) -> Vec<InterFunctionAssociated<'db>> {
        self.src(db)
            .into_type(db)
            .impls(db)
            .iter()
            .flat_map(|impl_| impl_.functions(db))
            .filter_map(|function| function.as_callible(db))
            .map(|callable| InterFunctionAssociated::new(db, callable))
            .collect()
    }

    /// A synthetic function that identifies current tag of a class.
    pub fn discriminator(self, db: &'db dyn Db) -> InterFunctionDiscriminator<'db> {
        InterFunctionDiscriminator::new(db, self.src(db))
    }

    pub fn is_coercable(self, db: &'db dyn Db) -> bool {
        self.variants(db).iter().any(|v| v.coerce(db).is_some())
    }

    /// All functions defined by this class, including field accessors,
    /// discriminator function and methods.
    #[salsa::tracked(returns(ref))]
    pub fn all_functions(self, db: &'db dyn Db) -> Vec<InterFunction<'db>> {
        self.methods(db)
            .iter()
            .copied()
            .map(|function| function.into())
            .chain(self.fields(db).iter().map(|f| f.getter(db).into()))
            .chain(
                self.fields(db)
                    .iter()
                    .filter_map(|f| f.setter(db).map(|f| f.into())),
            )
            .chain(std::iter::once(self.discriminator(db).into()))
            .chain(
                self.variants(db)
                    .iter()
                    .filter_map(|v| v.coerce(db).map(|f| f.into())),
            )
            .collect()
    }
}

impl<'db> From<InterClassDiscriminated<'db>> for InterClass<'db> {
    fn from(value: InterClassDiscriminated<'db>) -> Self {
        InterClass::Discriminated(value)
    }
}

#[salsa::tracked(debug)]
pub struct InterClassDiscriminatedVariant<'db> {
    pub class: InterClassDiscriminated<'db>,
    pub src: VariantQualified<'db>,
    /// The value of the tag field for this variant.
    pub tag_value: u32,
}

impl<'db> InterClassDiscriminatedVariant<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).declaration(db).name(db)
    }

    /// Fields of this varaint.
    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = InterField<'db>> {
        self.src(db).fields(db).map(move |f| f.interface(db))
    }

    /// Coerce function for this variant.
    pub fn coerce(self, db: &'db dyn Db) -> Option<InterFunctionDiscriminatedCoerce<'db>> {
        self.src(db)
            .is_constructible(db)
            .then(|| InterFunctionDiscriminatedCoerce::new(db, self))
    }
}

#[salsa::interned(debug)]
pub struct InterField<'db> {
    pub src: FieldQualified<'db>,
}

impl<'db> InterField<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db FieldName {
        self.src(db).declaration(db).name(db)
    }

    pub fn getter(self, db: &'db dyn Db) -> InterFunctionGetter<'db> {
        InterFunctionGetter::new(db, self.src(db))
    }

    pub fn setter(self, db: &'db dyn Db) -> Option<InterFunctionSetter<'db>> {
        let src = self.src(db);
        (!src.is_readonly(db)).then(|| InterFunctionSetter::new(db, src))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum InterFunction<'db> {
    /// A function declared in the source API.
    Free(InterFunctionFree<'db>),
    /// A source-defined function in an inherent impl block.
    Associated(InterFunctionAssociated<'db>),
    /// A synthetic field-read operation.
    Getter(InterFunctionGetter<'db>),
    /// A synthetic field-write operation.
    Setter(InterFunctionSetter<'db>),
    /// A synthetic operation that identifies an enum variant.
    EnumDiscriminator(InterFunctionDiscriminator<'db>),
    /// A synthetic operation that coerces a plain struct to a class.
    PlainCoerce(InterFunctionPlainCoerce<'db>),
    /// A synthetic operation that coerces a discriminated struct to a class.
    DiscriminatedCoerce(InterFunctionDiscriminatedCoerce<'db>),
}

#[salsa::tracked]
impl<'db> InterFunction<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        match self {
            InterFunction::Free(f) => f.symbol(db),
            InterFunction::Associated(f) => f.symbol(db),
            InterFunction::Getter(g) => g.symbol(db),
            InterFunction::Setter(s) => s.symbol(db),
            InterFunction::EnumDiscriminator(e) => e.symbol(db),
            InterFunction::PlainCoerce(p) => p.symbol(db),
            InterFunction::DiscriminatedCoerce(d) => d.symbol(db),
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<InterType<'db>> {
        match self {
            InterFunction::Free(f) => f.ret_type(db),
            InterFunction::Associated(f) => f.ret_type(db),
            InterFunction::Getter(g) => Some(g.ret_type(db)),
            InterFunction::Setter(_) => None,
            InterFunction::EnumDiscriminator(d) => Some(d.ret_type()),
            InterFunction::PlainCoerce(p) => Some(p.ret_type(db)),
            InterFunction::DiscriminatedCoerce(d) => Some(d.ret_type(db)),
        }
    }
    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<InterParam<'db>> {
        match self {
            InterFunction::Free(function) => function.params(db).clone(),
            InterFunction::Associated(function) => function.params(db).clone(),
            InterFunction::Getter(getter) => getter.params(db).to_vec(),
            InterFunction::Setter(setter) => setter.params(db).to_vec(),
            InterFunction::EnumDiscriminator(discriminator) => discriminator.params(db).to_vec(),
            InterFunction::PlainCoerce(coerce) => coerce.params(db).collect(),
            InterFunction::DiscriminatedCoerce(coerce) => coerce.params(db).collect(),
        }
    }
}

#[salsa::interned(debug)]
pub struct InterFunctionFree<'db> {
    pub src: Callable<'db>,
}

#[salsa::tracked]
impl<'db> InterFunctionFree<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__free__{}", self.src(db).path(db).mangled(db))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<InterType<'db>> {
        self.src(db).ret_type(db).map(|ret| ret.interface_out(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<InterParam<'db>> {
        let function = self.into();

        self.src(db)
            .params(db)
            .iter()
            .enumerate()
            .map(|(idx, src)| {
                let src_type = src.type_(db);
                let interface_type = src_type.interface_in(db);
                InterParam::new(db, function, idx, *src_type, interface_type)
            })
            .collect()
    }
}

impl<'db> From<InterFunctionFree<'db>> for InterFunction<'db> {
    fn from(value: InterFunctionFree<'db>) -> Self {
        InterFunction::Free(value)
    }
}

#[salsa::interned(debug)]
pub struct InterFunctionAssociated<'db> {
    pub src: Callable<'db>,
}

#[salsa::tracked]
impl<'db> InterFunctionAssociated<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__associated__{}", self.src(db).path(db).mangled(db))
    }

    #[salsa::tracked]
    pub fn self_class(self, db: &'db dyn Db) -> InterClass<'db> {
        let FunctionQualified::Inherent(function) = self.src(db).function(db) else {
            unreachable!("associated interface functions must be inherent")
        };
        match function.impl_(db).target(db).interface_in(db) {
            InterType::Class { inner, .. } => inner,
            _ => unreachable!("associated interface functions must target a class"),
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<InterType<'db>> {
        self.src(db).ret_type(db).map(|ret| ret.interface_out(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<InterParam<'db>> {
        if let Some(src_type) = self.src(db).receiver_type(db) {
            std::iter::once(InterParam::new(
                db,
                self.into(),
                0,
                src_type,
                src_type.interface_in(db),
            ))
            .chain(self.params_without_self(db).iter().copied())
            .collect()
        } else {
            self.params_without_self(db).clone()
        }
    }

    /// The parameters of this function, excluding the self (receiver)
    /// parameter.
    #[salsa::tracked(returns(ref))]
    pub fn params_without_self(self, db: &'db dyn Db) -> Vec<InterParam<'db>> {
        let offset = usize::from(self.src(db).declaration(db).receiver(db).is_some());

        self.src(db)
            .params(db)
            .iter()
            .enumerate()
            .map(move |(idx, src)| {
                let src_type = *src.type_(db);
                InterParam::new(
                    db,
                    self.into(),
                    idx + offset,
                    src_type,
                    src_type.interface_in(db),
                )
            })
            .collect()
    }
}

impl<'db> From<InterFunctionAssociated<'db>> for InterFunction<'db> {
    fn from(value: InterFunctionAssociated<'db>) -> Self {
        InterFunction::Associated(value)
    }
}

#[salsa::tracked(debug)]
pub struct InterParam<'db> {
    pub parent: InterFunction<'db>,
    pub idx: usize,
    #[tracked]
    pub src_type: Type<'db>,
    #[tracked]
    pub type_: InterType<'db>,
}

#[salsa::interned(debug)]
pub struct InterFunctionGetter<'db> {
    pub src: FieldQualified<'db>,
}

#[salsa::tracked]
impl<'db> InterFunctionGetter<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        let src = self.src(db);
        let parent_name = match src.target(db).kind(db) {
            TypeKind::Struct(type_) => type_.path(db).mangled(db),
            TypeKind::Enum(type_) => type_.path(db).mangled(db),
            _ => unreachable!("fields belong to classes"),
        };

        format!(
            "__fn__getter__{}__{}",
            parent_name,
            src.declaration(db).mangled_name(db)
        )
    }

    pub fn ret_type(self, db: &'db dyn Db) -> InterType<'db> {
        self.src(db)._type(db).interface_out(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> [InterParam<'db>; 1] {
        let field = self.src(db);
        let class_src_type = field.target(db);
        let class_interface_type = class_src_type.interface_in(db);

        [InterParam::new(
            db,
            self.into(),
            0,
            class_src_type,
            class_interface_type,
        )]
    }
}

impl<'db> From<InterFunctionGetter<'db>> for InterFunction<'db> {
    fn from(value: InterFunctionGetter<'db>) -> Self {
        InterFunction::Getter(value)
    }
}

#[salsa::interned(debug)]
pub struct InterFunctionSetter<'db> {
    pub src: FieldQualified<'db>,
}

#[salsa::tracked]
impl<'db> InterFunctionSetter<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        let src = self.src(db);
        let parent_name = match src.target(db).kind(db) {
            TypeKind::Struct(type_) => type_.path(db).mangled(db),
            TypeKind::Enum(type_) => type_.path(db).mangled(db),
            _ => unreachable!("fields belong to classes"),
        };

        format!(
            "__fn__setter__{}__{}",
            parent_name,
            src.declaration(db).mangled_name(db)
        )
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> [InterParam<'db>; 2] {
        let field = self.src(db);
        let class_src_type = field.target(db);
        let class_interface_type = class_src_type.interface_in(db);
        let value_src_type = field._type(db);
        let value_interface_type = value_src_type.interface_in(db);
        let function = self.into();

        [
            InterParam::new(db, function, 0, class_src_type, class_interface_type),
            InterParam::new(db, function, 1, value_src_type, value_interface_type),
        ]
    }
}

impl<'db> From<InterFunctionSetter<'db>> for InterFunction<'db> {
    fn from(value: InterFunctionSetter<'db>) -> Self {
        InterFunction::Setter(value)
    }
}

#[salsa::interned(debug)]
pub struct InterFunctionDiscriminator<'db> {
    pub src: EnumType<'db>,
}

#[salsa::tracked]
impl<'db> InterFunctionDiscriminator<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__disc__{}", self.src(db).path(db).mangled(db))
    }

    pub fn ret_type(self) -> InterType<'db> {
        InterType::f64
    }

    pub fn class(self, db: &'db dyn Db) -> InterClassDiscriminated<'db> {
        self.src(db).interface(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> [InterParam<'db>; 1] {
        let src_type = self.src(db).into_type(db);

        [InterParam::new(
            db,
            self.into(),
            0,
            src_type,
            src_type.interface_in(db),
        )]
    }
}

impl<'db> From<InterFunctionDiscriminator<'db>> for InterFunction<'db> {
    fn from(value: InterFunctionDiscriminator<'db>) -> Self {
        InterFunction::EnumDiscriminator(value)
    }
}

#[salsa::interned(debug)]
pub struct InterFunctionPlainCoerce<'db> {
    pub class: InterClassPlain<'db>,
}

#[salsa::tracked]
impl<'db> InterFunctionPlainCoerce<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__coe__{}", self.class(db).src(db).path(db).mangled(db))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> InterType<'db> {
        InterType::Class {
            is_borrowed: false,
            inner: self.class(db).into(),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn field_param_coerce(self, db: &'db dyn Db) -> Vec<(FieldName, InterParam<'db>)> {
        self.class(db)
            .src(db)
            .fields(db)
            .enumerate()
            .map(move |(idx, field)| {
                let type_ = field._type(db);
                (
                    field.declaration(db).name(db).clone(),
                    InterParam::new(db, self.into(), idx, type_, type_.interface_in(db)),
                )
            })
            .collect()
    }

    pub fn params(self, db: &'db dyn Db) -> impl Iterator<Item = InterParam<'db>> + 'db {
        self.field_param_coerce(db).iter().map(|(_, param)| *param)
    }
}

impl<'db> From<InterFunctionPlainCoerce<'db>> for InterFunction<'db> {
    fn from(value: InterFunctionPlainCoerce<'db>) -> Self {
        InterFunction::PlainCoerce(value)
    }
}

#[salsa::interned(debug)]
pub struct InterFunctionDiscriminatedCoerce<'db> {
    pub variant: InterClassDiscriminatedVariant<'db>,
}

#[salsa::tracked]
impl<'db> InterFunctionDiscriminatedCoerce<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!(
            "__fn__coe__{}",
            self.variant(db).src(db).path(db).mangled(db)
        )
    }

    pub fn ret_type(self, db: &'db dyn Db) -> InterType<'db> {
        InterType::Class {
            is_borrowed: false,
            inner: self.variant(db).class(db).into(),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn field_to_params(self, db: &'db dyn Db) -> Vec<(FieldName, InterParam<'db>)> {
        self.variant(db)
            .src(db)
            .fields(db)
            .enumerate()
            .map(move |(idx, field)| {
                let type_ = field._type(db);
                (
                    field.declaration(db).name(db).clone(),
                    InterParam::new(db, self.into(), idx, type_, type_.interface_in(db)),
                )
            })
            .collect()
    }

    pub fn params(self, db: &'db dyn Db) -> impl Iterator<Item = InterParam<'db>> + 'db {
        self.field_to_params(db).iter().map(|(_, param)| *param)
    }
}

impl<'db> From<InterFunctionDiscriminatedCoerce<'db>> for InterFunction<'db> {
    fn from(value: InterFunctionDiscriminatedCoerce<'db>) -> Self {
        InterFunction::DiscriminatedCoerce(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum InterType<'db> {
    f64,
    String,
    bool,
    Class {
        is_borrowed: bool,
        inner: InterClass<'db>,
    },
}
