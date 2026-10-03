//! The main intermediate representation used by rust-curse.
//!
//! Shim IR is the bridge between the supported Rust source API and the
//! generated JavaScript-facing API. It is used directly to generate the Rust
//! shim code and is also the source representation lowered into the facade and
//! FFI IRs.
//!
//! Unlike the downstream IRs, shim entities deliberately keep references to
//! the higher-level Rust IR public.

pub mod ext;
mod lowering;

use std::collections::VecDeque;

use crate::compiler::{
    db::Db,
    ir::rust::{
        Callable, Crate, EnumType, FieldName, FieldQualified, FunctionQualified, Module,
        ModuleParent, Rust, StructType, Type, TypeKind, VariantQualified, usages::UsageGraph,
    },
};

/// A root of the shim IR.
///
/// It's generally redundant API, generally serving as a quality-of-life wrapper
/// for tests.
#[salsa::interned(debug)]
pub struct Shim<'db> {
    pub rust: Rust<'db>,
    pub usages: UsageGraph<'db>,
}

impl<'db> Shim<'db> {
    pub fn crates(self, db: &'db dyn Db) -> impl Iterator<Item = ShimCrate<'db>> {
        self.rust(db)
            .crates(db)
            .iter()
            .map(move |crate_| crate_.shim_reachable_from(db, self.usages(db)))
    }
}

/// A shim crate, containing only reachable modules from the [`UsageGraph`].
#[salsa::interned(debug)]
pub struct ShimCrate<'db> {
    pub src: Crate<'db>,
    pub usages: UsageGraph<'db>,
}

impl<'db> ShimCrate<'db> {
    /// Whether this crate belongs to the configured set of local crates.
    pub fn is_local(self, db: &'db dyn Db) -> bool {
        self.src(db).is_local(db)
    }

    /// The root module of the crate.
    pub fn root(self, db: &'db dyn Db) -> ShimModule<'db> {
        self.src(db)
            .root(db)
            .shim_reachable_from(db, self.usages(db))
    }

    /// The root module and all reachable descendants, in breadth-first order.
    pub fn all_modules(self, db: &'db dyn Db) -> impl Iterator<Item = ShimModule<'db>> + 'db {
        let mut pending = VecDeque::from([self.root(db)]);
        std::iter::from_fn(move || {
            let module = pending.pop_front()?;
            pending.extend(module.modules(db));
            Some(module)
        })
    }
}

/// A shim crate used as a parent of the module.
///
/// The parent crate deliberately does not provide root module, they should be
/// obtained by converting it into a reachable [`ShimCrate`].
#[salsa::interned(debug)]
pub struct ShimParentCrate<'db> {
    pub src: Crate<'db>,
}

impl<'db> ShimParentCrate<'db> {
    pub fn into_reachable(self, db: &'db dyn Db, usages: UsageGraph<'db>) -> ShimCrate<'db> {
        ShimCrate::new(db, self.src(db), usages)
    }
}

/// A shim module used as a parent of other shim entity.
///
/// The parent module deliberately does not provide members, they should be
/// obtained by converting it into a reachable [`ShimModule`].
#[salsa::interned(debug)]
pub struct ShimParentModule<'db> {
    pub src: Module<'db>,
}

#[salsa::tracked]
impl<'db> ShimParentModule<'db> {
    pub fn parent(self, db: &'db dyn Db) -> ShimParent<'db> {
        match self.src(db).parent(db) {
            ModuleParent::Module(module) => ShimParent::Module(module.shim_parent(db)),
            ModuleParent::Crate(crate_) => ShimParent::Crate(crate_.shim_parent(db)),
        }
    }

    pub fn into_reachable(self, db: &'db dyn Db, usages: UsageGraph<'db>) -> ShimModule<'db> {
        ShimModule::new(db, self.src(db), usages)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ShimParent<'db> {
    Module(ShimParentModule<'db>),
    Crate(ShimParentCrate<'db>),
}

/// A shim module derived from a rust module, containing only reachable members
/// from the [`UsageGraph`].
#[salsa::interned(debug)]
pub struct ShimModule<'db> {
    pub src: Module<'db>,
    pub usages: UsageGraph<'db>,
}

#[salsa::tracked]
impl<'db> ShimModule<'db> {
    pub fn as_parent(self, db: &'db dyn Db) -> ShimParentModule<'db> {
        self.src(db).shim_parent(db)
    }

    pub fn members(self, db: &'db dyn Db) -> impl Iterator<Item = ShimMember<'db>> + 'db {
        self.lower_members(db).iter().cloned()
    }

    /// All reachable shim classes defined in the module.
    pub fn classes(self, db: &'db dyn Db) -> impl Iterator<Item = ShimClass<'db>> + 'db {
        self.members(db).filter_map(|member| match member {
            ShimMember::Class(class) => Some(class),
            _ => None,
        })
    }

    /// All reachable free functions defined in the module.
    pub fn functions(self, db: &'db dyn Db) -> impl Iterator<Item = ShimFunctionFree<'db>> + 'db {
        self.members(db).filter_map(|member| match member {
            ShimMember::Function(function) => Some(function),
            _ => None,
        })
    }

    /// All reachable child modules defined in the module.
    pub fn modules(self, db: &'db dyn Db) -> impl Iterator<Item = ShimModule<'db>> + 'db {
        self.members(db).filter_map(|member| match member {
            ShimMember::Module(module) => Some(module),
            _ => None,
        })
    }
}

/// A member reachable from a [`ShimModule`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ShimMember<'db> {
    Class(ShimClass<'db>),
    Function(ShimFunctionFree<'db>),
    Module(ShimModule<'db>),
}

/// A rust ADT represented as a class which is a more friendly representation
/// for a js.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ShimClass<'db> {
    Plain(ShimClassPlain<'db>),
    Discriminated(ShimClassDiscriminated<'db>),
}

impl<'db> ShimClass<'db> {
    /// The module this class belongs to.
    pub fn module(self, db: &'db dyn Db) -> ShimParentModule<'db> {
        match self {
            ShimClass::Plain(p) => p.module(db),
            ShimClass::Discriminated(d) => d.module(db),
        }
    }

    /// The name of this class.
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self {
            ShimClass::Plain(p) => p.name(db),
            ShimClass::Discriminated(d) => d.name(db),
        }
    }

    /// The symbol of this class.
    pub fn symbol(self, db: &'db dyn Db) -> &'db String {
        match self {
            ShimClass::Plain(p) => p.symbol(db),
            ShimClass::Discriminated(d) => d.symbol(db),
        }
    }

    /// The source type of this class.
    pub fn src_type(self, db: &'db dyn Db) -> Type<'db> {
        match self {
            ShimClass::Plain(p) => p.src(db).into_type(db),
            ShimClass::Discriminated(d) => d.src(db).into_type(db),
        }
    }

    /// Fields of this class, mapped from the source Rust ADT.
    pub fn fields(self, db: &'db dyn Db) -> &'db Vec<ShimField<'db>> {
        match self {
            ShimClass::Plain(p) => p.fields(db),
            ShimClass::Discriminated(d) => d.fields(db),
        }
    }

    /// Methods of this class.
    pub fn methods(self, db: &'db dyn Db) -> &'db Vec<ShimFunctionAssociated<'db>> {
        match self {
            ShimClass::Plain(p) => p.methods(db),
            ShimClass::Discriminated(d) => d.methods(db),
        }
    }

    /// All functions of this class, including associated functions,
    /// getters/setters and other synthetic functions.
    pub fn all_functions(self, db: &'db dyn Db) -> &'db Vec<ShimFunction<'db>> {
        match self {
            ShimClass::Plain(p) => p.all_functions(db),
            ShimClass::Discriminated(d) => d.all_functions(db),
        }
    }
}

/// A POD-like class.
#[salsa::interned(debug)]
pub struct ShimClassPlain<'db> {
    pub src: StructType<'db>,
}

#[salsa::tracked]
impl<'db> ShimClassPlain<'db> {
    /// The module this class belongs to.
    pub fn module(self, db: &'db dyn Db) -> ShimParentModule<'db> {
        self.src(db).def(db).module(db).shim_parent(db)
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
    pub fn fields(self, db: &'db dyn Db) -> Vec<ShimField<'db>> {
        self.src(db)
            .fields(db)
            .map(|field| field.shim(db))
            .collect()
    }

    /// Methods of this class.
    #[salsa::tracked(returns(ref))]
    pub fn methods(self, db: &'db dyn Db) -> Vec<ShimFunctionAssociated<'db>> {
        self.src(db)
            .into_type(db)
            .lower_methods(db)
            .iter()
            .copied()
            .filter(|method| !method.is_shadowed_by_field(db))
            .collect()
    }

    pub fn coerce(self, db: &'db dyn Db) -> Option<ShimFunctionPlainCoerce<'db>> {
        self.src(db)
            .is_constructible(db)
            .then(|| ShimFunctionPlainCoerce::new(db, self))
    }

    /// All functions defined by this class, including field accessors and
    /// methods.
    #[salsa::tracked(returns(ref))]
    pub fn all_functions(self, db: &'db dyn Db) -> Vec<ShimFunction<'db>> {
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

impl<'db> From<ShimClassPlain<'db>> for ShimClass<'db> {
    fn from(value: ShimClassPlain<'db>) -> Self {
        ShimClass::Plain(value)
    }
}

/// A tagged discriminated union class.
#[salsa::interned(debug)]
pub struct ShimClassDiscriminated<'db> {
    pub src: EnumType<'db>,
}

#[salsa::tracked]
impl<'db> ShimClassDiscriminated<'db> {
    /// The module this class belongs to.
    pub fn module(self, db: &'db dyn Db) -> ShimParentModule<'db> {
        self.src(db).def(db).module(db).shim_parent(db)
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
    pub fn variants(self, db: &'db dyn Db) -> Vec<ShimClassDiscriminatedVariant<'db>> {
        self.src(db)
            .variants(db)
            .enumerate()
            .map(|(i, variant)| ShimClassDiscriminatedVariant::new(db, self, variant, i as u32))
            .collect()
    }

    /// All fields of this class combined
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<ShimField<'db>> {
        self.variants(db)
            .iter()
            .flat_map(|v| v.fields(db))
            .collect()
    }

    /// Methods of this class.
    #[salsa::tracked(returns(ref))]
    pub fn methods(self, db: &'db dyn Db) -> Vec<ShimFunctionAssociated<'db>> {
        self.src(db)
            .into_type(db)
            .lower_methods(db)
            .iter()
            .copied()
            .filter(|method| !method.is_shadowed_by_field(db))
            .collect()
    }

    /// A synthetic function that identifies current tag of a class.
    pub fn discriminator(self, db: &'db dyn Db) -> ShimFunctionDiscriminator<'db> {
        ShimFunctionDiscriminator::new(db, self.src(db))
    }

    pub fn is_coercable(self, db: &'db dyn Db) -> bool {
        self.variants(db).iter().any(|v| v.coerce(db).is_some())
    }

    /// All functions defined by this class, including field accessors,
    /// discriminator function and methods.
    #[salsa::tracked(returns(ref))]
    pub fn all_functions(self, db: &'db dyn Db) -> Vec<ShimFunction<'db>> {
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

impl<'db> From<ShimClassDiscriminated<'db>> for ShimClass<'db> {
    fn from(value: ShimClassDiscriminated<'db>) -> Self {
        ShimClass::Discriminated(value)
    }
}

#[salsa::tracked(debug)]
pub struct ShimClassDiscriminatedVariant<'db> {
    pub class: ShimClassDiscriminated<'db>,
    pub src: VariantQualified<'db>,
    /// The value of the tag field for this variant.
    pub tag_value: u32,
}

impl<'db> ShimClassDiscriminatedVariant<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).declaration(db).name(db)
    }

    /// Fields of this varaint.
    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = ShimField<'db>> {
        self.src(db).fields(db).map(move |f| f.shim(db))
    }

    /// Coerce function for this variant.
    pub fn coerce(self, db: &'db dyn Db) -> Option<ShimFunctionDiscriminatedCoerce<'db>> {
        self.src(db)
            .is_constructible(db)
            .then(|| ShimFunctionDiscriminatedCoerce::new(db, self))
    }
}

#[salsa::interned(debug)]
pub struct ShimField<'db> {
    pub src: FieldQualified<'db>,
}

impl<'db> ShimField<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db FieldName {
        self.src(db).declaration(db).name(db)
    }

    pub fn getter(self, db: &'db dyn Db) -> ShimFunctionGetter<'db> {
        ShimFunctionGetter::new(db, self.src(db))
    }

    pub fn setter(self, db: &'db dyn Db) -> Option<ShimFunctionSetter<'db>> {
        let src = self.src(db);
        (!src.is_readonly(db)).then(|| ShimFunctionSetter::new(db, src))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ShimFunction<'db> {
    /// A function declared in the source API.
    Free(ShimFunctionFree<'db>),
    /// A source-defined function in an inherent impl block.
    Associated(ShimFunctionAssociated<'db>),
    /// A synthetic field-read operation.
    Getter(ShimFunctionGetter<'db>),
    /// A synthetic field-write operation.
    Setter(ShimFunctionSetter<'db>),
    /// A synthetic operation that identifies an enum variant.
    EnumDiscriminator(ShimFunctionDiscriminator<'db>),
    /// A synthetic operation that coerces a plain struct to a class.
    PlainCoerce(ShimFunctionPlainCoerce<'db>),
    /// A synthetic operation that coerces a discriminated struct to a class.
    DiscriminatedCoerce(ShimFunctionDiscriminatedCoerce<'db>),
}

#[salsa::tracked]
impl<'db> ShimFunction<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        match self {
            ShimFunction::Free(f) => f.symbol(db),
            ShimFunction::Associated(f) => f.symbol(db),
            ShimFunction::Getter(g) => g.symbol(db),
            ShimFunction::Setter(s) => s.symbol(db),
            ShimFunction::EnumDiscriminator(e) => e.symbol(db),
            ShimFunction::PlainCoerce(p) => p.symbol(db),
            ShimFunction::DiscriminatedCoerce(d) => d.symbol(db),
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimType<'db>> {
        match self {
            ShimFunction::Free(f) => f.ret_type(db),
            ShimFunction::Associated(f) => f.ret_type(db),
            ShimFunction::Getter(g) => Some(g.ret_type(db)),
            ShimFunction::Setter(_) => None,
            ShimFunction::EnumDiscriminator(d) => Some(d.ret_type()),
            ShimFunction::PlainCoerce(p) => Some(p.ret_type(db)),
            ShimFunction::DiscriminatedCoerce(d) => Some(d.ret_type(db)),
        }
    }
    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<ShimParam<'db>> {
        match self {
            ShimFunction::Free(function) => function.params(db).clone(),
            ShimFunction::Associated(function) => function.params(db).clone(),
            ShimFunction::Getter(getter) => getter.params(db).to_vec(),
            ShimFunction::Setter(setter) => setter.params(db).to_vec(),
            ShimFunction::EnumDiscriminator(discriminator) => discriminator.params(db).to_vec(),
            ShimFunction::PlainCoerce(coerce) => coerce.params(db).collect(),
            ShimFunction::DiscriminatedCoerce(coerce) => coerce.params(db).collect(),
        }
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionFree<'db> {
    pub src: Callable<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionFree<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__free__{}", self.src(db).path(db).mangled(db))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimType<'db>> {
        self.src(db).ret_type(db).map(|ret| ret.shim_out(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<ShimParam<'db>> {
        let function = self.into();

        self.src(db)
            .params(db)
            .iter()
            .enumerate()
            .map(|(idx, src)| {
                let src_type = src.type_(db);
                let shim_type = src_type.shim_in(db);
                ShimParam::new(db, function, idx, *src_type, shim_type)
            })
            .collect()
    }
}

impl<'db> From<ShimFunctionFree<'db>> for ShimFunction<'db> {
    fn from(value: ShimFunctionFree<'db>) -> Self {
        ShimFunction::Free(value)
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionAssociated<'db> {
    pub src: Callable<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionAssociated<'db> {
    pub fn is_shadowed_by_field(self, db: &'db dyn Db) -> bool {
        let src = self.src(db).declaration(db);
        src.receiver(db).is_some()
            && self.self_class(db).fields(db).iter().any(
                |field| matches!(field.name(db), FieldName::Name(name) if name == src.name(db)),
            )
    }

    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__associated__{}", self.src(db).path(db).mangled(db))
    }

    #[salsa::tracked]
    pub fn self_class(self, db: &'db dyn Db) -> ShimClass<'db> {
        let FunctionQualified::Inherent(function) = self.src(db).function(db) else {
            unreachable!("associated shim functions must be inherent")
        };
        match function.impl_(db).target(db).shim_in(db) {
            ShimType::Class { inner, .. } => inner,
            _ => unreachable!("associated shim functions must target a class"),
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimType<'db>> {
        self.src(db).ret_type(db).map(|ret| ret.shim_out(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<ShimParam<'db>> {
        if let Some(src_type) = self.src(db).receiver_type(db) {
            std::iter::once(ShimParam::new(
                db,
                self.into(),
                0,
                src_type,
                src_type.shim_in(db),
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
    pub fn params_without_self(self, db: &'db dyn Db) -> Vec<ShimParam<'db>> {
        let offset = usize::from(self.src(db).declaration(db).receiver(db).is_some());

        self.src(db)
            .params(db)
            .iter()
            .enumerate()
            .map(move |(idx, src)| {
                let src_type = *src.type_(db);
                ShimParam::new(
                    db,
                    self.into(),
                    idx + offset,
                    src_type,
                    src_type.shim_in(db),
                )
            })
            .collect()
    }
}

impl<'db> From<ShimFunctionAssociated<'db>> for ShimFunction<'db> {
    fn from(value: ShimFunctionAssociated<'db>) -> Self {
        ShimFunction::Associated(value)
    }
}

#[salsa::tracked(debug)]
pub struct ShimParam<'db> {
    pub parent: ShimFunction<'db>,
    pub idx: usize,
    #[tracked]
    pub src_type: Type<'db>,
    #[tracked]
    pub type_: ShimType<'db>,
}

#[salsa::interned(debug)]
pub struct ShimFunctionGetter<'db> {
    pub src: FieldQualified<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionGetter<'db> {
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

    pub fn ret_type(self, db: &'db dyn Db) -> ShimType<'db> {
        self.src(db)._type(db).shim_out(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> [ShimParam<'db>; 1] {
        let field = self.src(db);
        let class_src_type = field.target(db);
        let class_shim_type = class_src_type.shim_in(db);

        [ShimParam::new(
            db,
            self.into(),
            0,
            class_src_type,
            class_shim_type,
        )]
    }
}

impl<'db> From<ShimFunctionGetter<'db>> for ShimFunction<'db> {
    fn from(value: ShimFunctionGetter<'db>) -> Self {
        ShimFunction::Getter(value)
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionSetter<'db> {
    pub src: FieldQualified<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionSetter<'db> {
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
    pub fn params(self, db: &'db dyn Db) -> [ShimParam<'db>; 2] {
        let field = self.src(db);
        let class_src_type = field.target(db);
        let class_shim_type = class_src_type.shim_in(db);
        let value_src_type = field._type(db);
        let value_shim_type = value_src_type.shim_in(db);
        let function = self.into();

        [
            ShimParam::new(db, function, 0, class_src_type, class_shim_type),
            ShimParam::new(db, function, 1, value_src_type, value_shim_type),
        ]
    }
}

impl<'db> From<ShimFunctionSetter<'db>> for ShimFunction<'db> {
    fn from(value: ShimFunctionSetter<'db>) -> Self {
        ShimFunction::Setter(value)
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionDiscriminator<'db> {
    pub src: EnumType<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionDiscriminator<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__disc__{}", self.src(db).path(db).mangled(db))
    }

    pub fn ret_type(self) -> ShimType<'db> {
        ShimType::f64
    }

    pub fn class(self, db: &'db dyn Db) -> ShimClassDiscriminated<'db> {
        self.src(db).shim(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> [ShimParam<'db>; 1] {
        let src_type = self.src(db).into_type(db);

        [ShimParam::new(
            db,
            self.into(),
            0,
            src_type,
            src_type.shim_in(db),
        )]
    }
}

impl<'db> From<ShimFunctionDiscriminator<'db>> for ShimFunction<'db> {
    fn from(value: ShimFunctionDiscriminator<'db>) -> Self {
        ShimFunction::EnumDiscriminator(value)
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionPlainCoerce<'db> {
    pub class: ShimClassPlain<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionPlainCoerce<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__coe__{}", self.class(db).src(db).path(db).mangled(db))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> ShimType<'db> {
        ShimType::Class {
            is_borrowed: false,
            inner: self.class(db).into(),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn field_param_coerce(self, db: &'db dyn Db) -> Vec<(FieldName, ShimParam<'db>)> {
        self.class(db)
            .src(db)
            .fields(db)
            .enumerate()
            .map(move |(idx, field)| {
                let type_ = field._type(db);
                (
                    field.declaration(db).name(db).clone(),
                    ShimParam::new(db, self.into(), idx, type_, type_.shim_in(db)),
                )
            })
            .collect()
    }

    pub fn params(self, db: &'db dyn Db) -> impl Iterator<Item = ShimParam<'db>> + 'db {
        self.field_param_coerce(db).iter().map(|(_, param)| *param)
    }
}

impl<'db> From<ShimFunctionPlainCoerce<'db>> for ShimFunction<'db> {
    fn from(value: ShimFunctionPlainCoerce<'db>) -> Self {
        ShimFunction::PlainCoerce(value)
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionDiscriminatedCoerce<'db> {
    pub variant: ShimClassDiscriminatedVariant<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionDiscriminatedCoerce<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!(
            "__fn__coe__{}",
            self.variant(db).src(db).path(db).mangled(db)
        )
    }

    pub fn ret_type(self, db: &'db dyn Db) -> ShimType<'db> {
        ShimType::Class {
            is_borrowed: false,
            inner: self.variant(db).class(db).into(),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn field_to_params(self, db: &'db dyn Db) -> Vec<(FieldName, ShimParam<'db>)> {
        self.variant(db)
            .src(db)
            .fields(db)
            .enumerate()
            .map(move |(idx, field)| {
                let type_ = field._type(db);
                (
                    field.declaration(db).name(db).clone(),
                    ShimParam::new(db, self.into(), idx, type_, type_.shim_in(db)),
                )
            })
            .collect()
    }

    pub fn params(self, db: &'db dyn Db) -> impl Iterator<Item = ShimParam<'db>> + 'db {
        self.field_to_params(db).iter().map(|(_, param)| *param)
    }
}

impl<'db> From<ShimFunctionDiscriminatedCoerce<'db>> for ShimFunction<'db> {
    fn from(value: ShimFunctionDiscriminatedCoerce<'db>) -> Self {
        ShimFunction::DiscriminatedCoerce(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum ShimType<'db> {
    f64,
    String,
    bool,
    Class {
        is_borrowed: bool,
        inner: ShimClass<'db>,
    },
}
