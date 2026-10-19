//! Intermediate representation of the generated Rust runtime.
//!
//! It describes what the generated Rust code contains for an interface: the
//! exported operations, including synthetic ones such as field accessors,
//! discriminators and coerce functions, and their symbols.

pub mod ext;
pub mod tethering;

use itertools::Either;

use crate::compiler::{
    db::Db,
    ir::{
        abi::AbiReturn,
        facade::{
            Facade, FacadeClass, FacadeClassDiscriminated, FacadeClassPlain, FacadeFieldPlain,
            FacadeFreeFunction, FacadeMethod, FacadeVariant,
        },
        runtime::tethering::RuntimeTethering,
        rust::{Type, TypeKind},
    },
};

/// The runtime of a reachable interface API.
#[salsa::interned(debug)]
pub struct Runtime<'db> {
    pub facade: Facade<'db>,
}

#[salsa::tracked]
impl<'db> Runtime<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn imports_js(self, db: &'db dyn Db) -> Vec<RuntimeExport> {
        if self
            .functions(db)
            .iter()
            .any(|function| matches!(function.abi_ret(db), AbiReturn::Union(_)))
        {
            vec![RuntimeExport::RuntimeError]
        } else {
            vec![]
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn tetherings(self, db: &'db dyn Db) -> Vec<RuntimeTethering<'db>> {
        self.facade(db)
            .crates(db)
            .flat_map(|crate_| crate_.all_modules(db))
            .flat_map(|module| module.classes(db))
            .map(|class| class.tethering(db))
            .collect()
    }

    /// All functions exported by the runtime, including methods, field
    /// accessors and other synthetic functions.
    #[salsa::tracked(returns(ref))]
    pub fn functions(self, db: &'db dyn Db) -> Vec<RuntimeFunction<'db>> {
        self.facade(db)
            .crates(db)
            .flat_map(|crate_| crate_.all_modules(db))
            .flat_map(|module| {
                module.functions(db).map(RuntimeFunction::from).chain(
                    module
                        .classes(db)
                        .flat_map(|class| class_functions(db, class)),
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub struct RuntimeModule;

impl RuntimeModule {
    pub fn virtual_path(self) -> &'static str {
        "$rust"
    }

    pub fn resolve(path: &str) -> Option<Self> {
        (path == Self.virtual_path()).then_some(Self)
    }

    pub fn exports(self) -> &'static [RuntimeExport] {
        &[RuntimeExport::RuntimeError]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum RuntimeExport {
    RuntimeError,
}

impl RuntimeExport {
    pub fn module(self) -> RuntimeModule {
        match self {
            Self::RuntimeError => RuntimeModule,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::RuntimeError => "RuntimeError",
        }
    }

    pub fn import_alias(self) -> &'static str {
        match self {
            Self::RuntimeError => "$runtime__RuntimeError",
        }
    }
}

fn class_functions<'db>(
    db: &'db dyn Db,
    class: FacadeClass<'db>,
) -> impl Iterator<Item = RuntimeFunction<'db>> + 'db {
    let methods = class.methods(db).map(RuntimeFunction::from);
    let getters = class.source_fields(db).iter().map(|f| f.getter(db).into());
    let setters = class
        .source_fields(db)
        .iter()
        .filter_map(|f| f.setter(db).map(RuntimeFunction::from));

    let synthetic = match class {
        FacadeClass::Plain(class) => Either::Left(
            class
                .runtime_coerce(db)
                .map(RuntimeFunction::from)
                .into_iter(),
        ),
        FacadeClass::Discriminated(class) => Either::Right(
            std::iter::once(class.runtime_discriminator(db).into()).chain(
                class
                    .variants(db)
                    .iter()
                    .filter_map(|v| v.runtime_coerce(db).map(RuntimeFunction::from)),
            ),
        ),
    };

    methods
        .chain(getters)
        .chain(setters)
        .chain(synthetic)
        .chain(std::iter::once(RuntimeFunction::DropHandle(
            class.drop_handle_fn(db),
        )))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum RuntimeFunction<'db> {
    /// A function declared in the source API.
    Free(FacadeFreeFunction<'db>),
    /// A source-defined function in an inherent impl block.
    Associated(FacadeMethod<'db>),
    /// A synthetic field-read operation.
    Getter(RuntimeGetter<'db>),
    /// A synthetic field-write operation.
    Setter(RuntimeSetter<'db>),
    /// A synthetic operation that identifies an enum variant.
    Discriminator(RuntimeDiscriminator<'db>),
    /// A synthetic operation that coerces a plain struct to a class.
    PlainCoerce(RuntimePlainCoerce<'db>),
    /// A synthetic operation that coerces a discriminated struct to a class.
    DiscriminatedCoerce(RuntimeDiscriminatedCoerce<'db>),
    DropHandle(RuntimeDropHandle<'db>),
}

#[salsa::tracked]
impl<'db> RuntimeFunction<'db> {
    /// The mangled unique symbol name of this function.
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        match self {
            Self::Free(function) => function.symbol(db),
            Self::Associated(function) => function.symbol(db),
            Self::Getter(getter) => getter.symbol(db),
            Self::Setter(setter) => setter.symbol(db),
            Self::Discriminator(discriminator) => discriminator.symbol(db),
            Self::PlainCoerce(coerce) => coerce.symbol(db),
            Self::DiscriminatedCoerce(coerce) => coerce.symbol(db),
            Self::DropHandle(drop) => drop.symbol(db),
        }
    }

    /// The exact Rust types of the parameters of this function.
    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<Type<'db>> {
        match self {
            Self::Free(function) => function.params(db).iter().map(|p| p.src_type(db)).collect(),
            Self::Associated(function) => function
                .src(db)
                .receiver_type(db)
                .into_iter()
                .chain(function.params(db).iter().map(|p| p.src_type(db)))
                .collect(),
            Self::Getter(getter) => vec![getter.field(db).src(db).target(db)],
            Self::Setter(setter) => {
                let field = setter.field(db).src(db);

                vec![field.target(db), field._type(db)]
            }
            Self::Discriminator(discriminator) => {
                vec![discriminator.class(db).src(db).into_type(db)]
            }
            Self::PlainCoerce(coerce) => coerce
                .class(db)
                .src(db)
                .fields(db)
                .map(|field| field._type(db))
                .collect(),
            Self::DiscriminatedCoerce(coerce) => coerce
                .variant(db)
                .src(db)
                .fields(db)
                .map(|field| field._type(db))
                .collect(),
            Self::DropHandle(drop) => vec![drop.tethering(db).source(db).src_type(db)],
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> RuntimeType<'db> {
        let type_ = match self {
            Self::Free(function) => function.src(db).ret_type(db),
            Self::Associated(function) => function.src(db).ret_type(db),
            Self::Getter(getter) => {
                return getter.field(db).tethered(db).value_type(db);
            }
            Self::Setter(_) | Self::DropHandle(_) => {
                return RuntimeType::Unit;
            }
            Self::Discriminator(_) => return RuntimeType::u32,
            Self::PlainCoerce(coerce) => {
                return RuntimeType::Source(coerce.class(db).src(db).into_type(db));
            }
            Self::DiscriminatedCoerce(coerce) => {
                return RuntimeType::Source(coerce.variant(db).class(db).src(db).into_type(db));
            }
        };

        match type_.as_result_variants(db) {
            Some((ok, err)) => RuntimeType::Result { ok, err },
            None => RuntimeType::Source(type_),
        }
    }
}

#[salsa::interned(debug)]
pub struct RuntimeDropHandle<'db> {
    pub tethering: RuntimeTethering<'db>,
}

#[salsa::tracked]
impl<'db> RuntimeDropHandle<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        let source = self.tethering(db).source(db);
        let path = match source {
            FacadeClass::Plain(class) => class.src(db).path(db),
            FacadeClass::Discriminated(class) => class.src(db).path(db),
        };

        format!("__fn__drop_handle__{}", path.mangled(db))
    }
}

/// A type used by the generated Rust runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum RuntimeType<'db> {
    Unit,
    /// A value of an exact Rust type.
    Source(Type<'db>),
    /// A "synthetic" u32 type, without the hir provenance.
    u32,
    Tethered(RuntimeTethering<'db>),
    Result {
        ok: Type<'db>,
        err: Type<'db>,
    },
}

impl<'db> From<FacadeFreeFunction<'db>> for RuntimeFunction<'db> {
    fn from(value: FacadeFreeFunction<'db>) -> Self {
        Self::Free(value)
    }
}

impl<'db> From<FacadeMethod<'db>> for RuntimeFunction<'db> {
    fn from(value: FacadeMethod<'db>) -> Self {
        Self::Associated(value)
    }
}

#[salsa::tracked]
impl<'db> FacadeFreeFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__free__{}", self.src(db).path(db).mangled(db))
    }
}

#[salsa::tracked]
impl<'db> FacadeMethod<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__associated__{}", self.src(db).path(db).mangled(db))
    }
}

#[salsa::interned(debug)]
pub struct RuntimeGetter<'db> {
    pub field: FacadeFieldPlain<'db>,
}

#[salsa::tracked]
impl<'db> RuntimeGetter<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        let src = self.field(db).src(db);
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
}

impl<'db> From<RuntimeGetter<'db>> for RuntimeFunction<'db> {
    fn from(value: RuntimeGetter<'db>) -> Self {
        Self::Getter(value)
    }
}

#[salsa::interned(debug)]
pub struct RuntimeSetter<'db> {
    pub field: FacadeFieldPlain<'db>,
}

#[salsa::tracked]
impl<'db> RuntimeSetter<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        let src = self.field(db).src(db);
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
}

impl<'db> From<RuntimeSetter<'db>> for RuntimeFunction<'db> {
    fn from(value: RuntimeSetter<'db>) -> Self {
        Self::Setter(value)
    }
}

#[salsa::interned(debug)]
pub struct RuntimeDiscriminator<'db> {
    pub class: FacadeClassDiscriminated<'db>,
}

#[salsa::tracked]
impl<'db> RuntimeDiscriminator<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!(
            "__fn__disc__{}",
            self.class(db).src(db).path(db).mangled(db)
        )
    }
}

impl<'db> From<RuntimeDiscriminator<'db>> for RuntimeFunction<'db> {
    fn from(value: RuntimeDiscriminator<'db>) -> Self {
        Self::Discriminator(value)
    }
}

#[salsa::interned(debug)]
pub struct RuntimePlainCoerce<'db> {
    pub class: FacadeClassPlain<'db>,
}

#[salsa::tracked]
impl<'db> RuntimePlainCoerce<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__coe__{}", self.class(db).src(db).path(db).mangled(db))
    }
}

impl<'db> From<RuntimePlainCoerce<'db>> for RuntimeFunction<'db> {
    fn from(value: RuntimePlainCoerce<'db>) -> Self {
        Self::PlainCoerce(value)
    }
}

#[salsa::interned(debug)]
pub struct RuntimeDiscriminatedCoerce<'db> {
    pub variant: FacadeVariant<'db>,
}

#[salsa::tracked]
impl<'db> RuntimeDiscriminatedCoerce<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        format!(
            "__fn__coe__{}",
            self.variant(db).src(db).path(db).mangled(db)
        )
    }
}

impl<'db> From<RuntimeDiscriminatedCoerce<'db>> for RuntimeFunction<'db> {
    fn from(value: RuntimeDiscriminatedCoerce<'db>) -> Self {
        Self::DiscriminatedCoerce(value)
    }
}
