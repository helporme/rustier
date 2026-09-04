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

use crate::compiler::{
    db::Db,
    ir::rust::{
        Crate, Enum, Field, FieldName, FieldParent, Function, FunctionParent, IntoValueType,
        Module, Receiver, Rust, Struct, Type, ValueType, Variant,
    },
};

/// A root of the shim IR.
///
/// It's generally redundant API, generally serving as a quality-of-life wrapper
/// for tests.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, salsa::Update)]
pub struct Shim<'db> {
    pub rust: Rust<'db>,
}

impl<'db> Shim<'db> {
    pub fn crates(self, db: &'db dyn Db) -> impl Iterator<Item = ShimCrate<'db>> {
        self.rust.crates(db).iter().map(|crate_| crate_.shim(db))
    }
}

/// A shim crate matching the original Rust crate.
///
/// The crate isn't needed for shim codegen, but is retained for easier
/// lowering of js IRs.
#[salsa::interned(debug)]
pub struct ShimCrate<'db> {
    pub src: Crate<'db>,
}

impl<'db> ShimCrate<'db> {
    pub fn root(self, db: &'db dyn Db) -> ShimModule<'db> {
        self.src(db).root(db).shim(db)
    }
}

/// A shim module matching original rust module.
///
/// The module isn't needed for shim codegen, but is retained for easier
/// lowering of js IRs.
#[salsa::interned(debug)]
pub struct ShimModule<'db> {
    pub src: Module<'db>,
}

#[salsa::tracked]
impl<'db> ShimModule<'db> {
    /// Returns all shim members matching the original members defined in the
    /// rust module.
    #[salsa::tracked(returns(ref))]
    pub fn members(self, db: &'db dyn Db) -> Vec<ShimMember<'db>> {
        self.src(db)
            .members(db)
            .iter()
            .map(|member| member.shim(db))
            .collect()
    }

    /// Returns all shim classes defined in the module.
    pub fn classes(self, db: &'db dyn Db) -> impl Iterator<Item = ShimClass<'db>> + 'db {
        self.members(db).iter().filter_map(|m| match m {
            ShimMember::Class(c) => Some(*c),
            _ => None,
        })
    }

    /// Returns all shim functions defined in the module.
    ///
    /// Take into account that it doesn't return associated functions of classes
    /// or other synthetic functions.
    pub fn functions(self, db: &'db dyn Db) -> impl Iterator<Item = ShimFunctionFree<'db>> + 'db {
        self.members(db).iter().filter_map(|m| match m {
            ShimMember::Function(f) => Some(*f),
            _ => None,
        })
    }

    /// Returns all shim modules defined in the module.
    pub fn modules(self, db: &'db dyn Db) -> impl Iterator<Item = ShimModule<'db>> + 'db {
        self.members(db).iter().filter_map(|m| match m {
            ShimMember::Module(m) => Some(*m),
            _ => None,
        })
    }
}

/// A member defined in a shim module.
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
    pub fn module(self, db: &'db dyn Db) -> ShimModule<'db> {
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

    /// The source value type of this class.
    pub fn src_value_type(self, db: &'db dyn Db) -> ValueType<'db> {
        match self {
            ShimClass::Plain(p) => p.src(db).into_value_type(),
            ShimClass::Discriminated(d) => d.src(db).into_value_type(),
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
    pub src: Struct<'db>,
}

#[salsa::tracked]
impl<'db> ShimClassPlain<'db> {
    /// The module this class belongs to.
    pub fn module(self, db: &'db dyn Db) -> ShimModule<'db> {
        self.src(db).module(db).shim(db)
    }

    /// The name of this class.
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.src(db).name(db)
    }

    /// The symbol of this class.
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__plain__{}", self.src(db).path(db).mangled(db))
    }

    /// Fields of this class.
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<ShimField<'db>> {
        let fields = self.src(db).fields(db);
        let methods = self.methods(db);

        fields
            .iter()
            .filter(|field| {
                !methods.iter().any(|method| {
                    method.src(db).receiver(db).is_some()
                        && matches!(
                            field.name(db),
                            FieldName::Name(name) if name == method.src(db).name(db)
                        )
                })
            })
            .map(|field| field.shim(db))
            .collect()
    }

    /// Methods of this class.
    #[salsa::tracked(returns(ref))]
    pub fn methods(self, db: &'db dyn Db) -> Vec<ShimFunctionAssociated<'db>> {
        self.src(db)
            .impls(db)
            .iter()
            .flat_map(|impl_| impl_.lower_as_methods(db))
            .collect()
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
    pub src: Enum<'db>,
}

#[salsa::tracked]
impl<'db> ShimClassDiscriminated<'db> {
    /// The module this class belongs to.
    pub fn module(self, db: &'db dyn Db) -> ShimModule<'db> {
        self.src(db).module(db).shim(db)
    }

    /// The name of this class.
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.src(db).name(db)
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
            .iter()
            .enumerate()
            .map(|(i, variant)| ShimClassDiscriminatedVariant::new(db, *variant, i as u32))
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
            .impls(db)
            .iter()
            .flat_map(|impl_| impl_.lower_as_methods(db))
            .collect()
    }

    /// A synthetic function that identifies current tag of a class.
    pub fn discriminator(self, db: &'db dyn Db) -> ShimFunctionDiscriminator<'db> {
        ShimFunctionDiscriminator::new(db, self.src(db))
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
    pub src: Variant<'db>,
    /// The value of the tag field for this variant.
    pub tag_value: u32,
}

impl<'db> ShimClassDiscriminatedVariant<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).name(db)
    }

    /// Fields of this varaint.
    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = ShimField<'db>> {
        self.src(db).fields(db).iter().map(move |f| f.shim(db))
    }
}

#[salsa::interned(debug)]
pub struct ShimField<'db> {
    pub src: Field<'db>,
}

impl<'db> ShimField<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db FieldName {
        self.src(db).name(db)
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
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimType<'db>> {
        match self {
            ShimFunction::Free(f) => f.ret_type(db),
            ShimFunction::Associated(f) => f.ret_type(db),
            ShimFunction::Getter(g) => Some(g.ret_type(db)),
            ShimFunction::Setter(_) => None,
            ShimFunction::EnumDiscriminator(d) => Some(d.ret_type()),
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
        }
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionFree<'db> {
    pub src: Function<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionFree<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__free__{}", self.src(db).path(db).mangled(db))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimType<'db>> {
        self.src(db)
            .ret_type(db)
            .map(|ret| ret.as_type().shim_out(db))
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
    pub src: Function<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionAssociated<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__associated__{}", self.src(db).path(db).mangled(db))
    }

    #[salsa::tracked]
    pub fn self_class(self, db: &'db dyn Db) -> ShimClass<'db> {
        match self.src(db).parent(db) {
            FunctionParent::Impl(impl_) => impl_.target(db).shim(db),
            FunctionParent::Module(_) => {
                unreachable!("associated shim functions must belong to an impl block")
            }
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimType<'db>> {
        self.src(db)
            .ret_type(db)
            .map(|ret| ret.as_type().shim_out(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<ShimParam<'db>> {
        if let Some(receiver) = self.src(db).receiver(db) {
            let value_type = self.self_class(db).src_value_type(db);
            let src_type = match receiver {
                Receiver::Shared => value_type.borrowed(),
                Receiver::Exclusive => value_type.borrowed_mut(),
                Receiver::Owned => value_type.as_type(),
            };

            std::iter::once(ShimParam::new(
                db,
                self.into(),
                0,
                src_type,
                src_type.shim_in(db),
            ))
            .chain(self.params_without_self(db))
            .collect()
        } else {
            self.params_without_self(db).collect()
        }
    }

    /// Returns the parameters of this function, excluding the self (receiver)
    /// parameter.
    pub fn params_without_self(
        self,
        db: &'db dyn Db,
    ) -> impl Iterator<Item = ShimParam<'db>> + 'db {
        let offset = usize::from(self.src(db).receiver(db).is_some());

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
    pub src: Field<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionGetter<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        let src = self.src(db);
        let parent_name = match src.parent(db) {
            FieldParent::Struct(struct_) => struct_.path(db).mangled(db),
            FieldParent::Variant(variant) => variant.parent(db).path(db).mangled(db),
        };

        format!("__fn__getter__{}__{}", parent_name, src.mangled_name(db))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> ShimType<'db> {
        self.src(db).value_type(db).as_type().shim_out(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> [ShimParam<'db>; 1] {
        let field = self.src(db);
        let class = match field.parent(db) {
            FieldParent::Struct(struct_) => struct_.shim(db).into(),
            FieldParent::Variant(variant) => variant.parent(db).shim(db).into(),
        };
        let class_shim_type = ShimType::Class {
            is_borrowed: true,
            inner: class,
        };
        let class_src_type = class.src_value_type(db).as_type();

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
    pub src: Field<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionSetter<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        let src = self.src(db);
        let parent_name = match src.parent(db) {
            FieldParent::Struct(struct_) => struct_.path(db).mangled(db),
            FieldParent::Variant(variant) => variant.parent(db).path(db).mangled(db),
        };

        format!("__fn__setter__{}__{}", parent_name, src.mangled_name(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> [ShimParam<'db>; 2] {
        let field = self.src(db);
        let class = match field.parent(db) {
            FieldParent::Struct(struct_) => struct_.shim(db).into(),
            FieldParent::Variant(variant) => variant.parent(db).shim(db).into(),
        };
        let class_shim_type = ShimType::Class {
            is_borrowed: true,
            inner: class,
        };
        let class_src_type = class.src_value_type(db).into();
        let value_src_type = field.value_type(db).as_type();
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
    pub src: Enum<'db>,
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
        let src_type = Type::from(ValueType::from(self.src(db)));

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
