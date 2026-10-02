//! Intermediate representation used to generate TypeScript declarations.

pub mod ext;
mod lowering;

use std::collections::HashSet;

use itertools::Either;

use crate::compiler::{
    db::Db,
    ir::{
        rust::{Callable, FieldName, Span},
        shim::{
            Shim, ShimClassDiscriminated, ShimClassDiscriminatedVariant, ShimClassPlain, ShimField,
            ShimFunction, ShimFunctionAssociated, ShimFunctionDiscriminator, ShimFunctionFree,
            ShimMember, ShimModule, ShimParam, ShimParentModule, ShimType,
        },
    },
    path::FilePath,
};

/// TypeScript declarations for the reachable shim API.
#[salsa::interned(debug)]
pub struct Decals<'db> {
    shim: Shim<'db>,
}

#[salsa::tracked]
impl<'db> Decals<'db> {
    /// Reachable local modules, sorted by virtual path.
    #[salsa::tracked(returns(ref))]
    pub fn local_modules(self, db: &'db dyn Db) -> Vec<DecalModule<'db>> {
        let mut modules = self
            .shim(db)
            .crates(db)
            .filter(move |crate_| crate_.is_local(db))
            .flat_map(move |crate_| crate_.all_modules(db))
            .map(move |module| module.decals(db))
            .collect::<Vec<_>>();
        modules.sort_by_key(|module| module.virtual_path(db));
        modules
    }

    /// Reachable external modules, sorted by virtual path.
    #[salsa::tracked(returns(ref))]
    pub fn external_modules(self, db: &'db dyn Db) -> Vec<DecalModule<'db>> {
        let mut modules = self
            .shim(db)
            .crates(db)
            .filter(move |crate_| !crate_.is_local(db))
            .flat_map(move |crate_| crate_.all_modules(db))
            .map(move |module| module.decals(db))
            .collect::<Vec<_>>();
        modules.sort_by_key(|module| module.virtual_path(db));
        modules
    }
}

#[salsa::tracked(debug)]
pub struct DecalImport<'db> {
    pub originating_from: DecalModule<'db>,
    pub class: DecalClass<'db>,
}

/// A TypeScript declaration module used as a parent of another declaration
/// entity.
#[salsa::interned(debug)]
pub struct DecalParentModule<'db> {
    shim: ShimParentModule<'db>,
}

#[salsa::tracked]
impl<'db> DecalParentModule<'db> {
    pub fn is_local(self, db: &'db dyn Db) -> bool {
        self.shim(db).src(db).path(db).crate_(db).is_local(db)
    }

    pub fn virtual_path(self, db: &'db dyn Db) -> &'db String {
        self.shim(db).facade(db).virtual_path(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.shim(db).src(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.shim(db).src(db).span(db)
    }

    pub fn file_path(self, db: &'db dyn Db) -> &'db FilePath {
        self.shim(db).src(db).file_path(db)
    }
}

/// A reachable TypeScript declaration module.
#[salsa::interned(debug)]
pub struct DecalModule<'db> {
    shim: ShimModule<'db>,
}

#[salsa::tracked]
impl<'db> DecalModule<'db> {
    pub fn virtual_path(self, db: &'db dyn Db) -> &'db String {
        self.shim(db).facade(db).virtual_path(db)
    }

    pub fn as_parent(self, db: &'db dyn Db) -> DecalParentModule<'db> {
        self.shim(db).as_parent(db).decals(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.as_parent(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.as_parent(db).span(db)
    }

    /// Members defined in this module.
    pub fn members(self, db: &'db dyn Db) -> impl Iterator<Item = DecalMember<'db>> + 'db {
        self.shim(db).members(db).map(|member| match member {
            ShimMember::Class(class) => DecalMember::Class(class.decals(db)),
            ShimMember::Function(function) => DecalMember::Function(function.decals(db)),
            ShimMember::Module(module) => DecalMember::Module(module.decals(db)),
        })
    }

    /// Imports needed for this module.
    #[salsa::tracked(returns(ref))]
    pub fn imports(self, db: &'db dyn Db) -> Vec<DecalImport<'db>> {
        let mut classes = HashSet::new();

        for member in self.members(db) {
            match member {
                DecalMember::Function(function) => {
                    classes.extend(function.referenced_classes(db));
                }
                DecalMember::Class(class) => {
                    classes.extend(class.referenced_classes(db));
                }
                DecalMember::Module(_) => {}
            }
        }

        classes
            .into_iter()
            .filter(|class| class.module(db) != self.as_parent(db))
            .map(|class| DecalImport::new(db, self, class))
            .collect()
    }

    pub fn file_path(self, db: &'db dyn Db) -> &'db FilePath {
        self.as_parent(db).file_path(db)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum DecalMember<'db> {
    Class(DecalClass<'db>),
    Function(DecalFreeFunction<'db>),
    Module(DecalModule<'db>),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum DecalFunction<'db> {
    Free(DecalFreeFunction<'db>),
    Method(DecalMethod<'db>),
}

#[salsa::tracked]
impl<'db> DecalFunction<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        match self {
            Self::Free(function) => function.name(db),
            Self::Method(method) => method.name(db),
        }
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        match self {
            Self::Free(function) => function.span(db),
            Self::Method(method) => method.span(db),
        }
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        match self {
            Self::Free(function) => function.docs(db),
            Self::Method(method) => method.docs(db),
        }
    }

    pub fn params(self, db: &'db dyn Db) -> impl Iterator<Item = DecalParam<'db>> + 'db {
        match self {
            Self::Free(function) => Either::Left(function.params(db)),
            Self::Method(method) => Either::Right(method.params(db)),
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<DecalType<'db>> {
        match self {
            Self::Free(function) => function.ret_type(db),
            Self::Method(method) => method.ret_type(db),
        }
    }
}

#[salsa::interned(debug)]
pub struct DecalFreeFunction<'db> {
    shim: ShimFunctionFree<'db>,
}

#[salsa::tracked]
impl<'db> DecalFreeFunction<'db> {
    pub fn as_fn(self) -> DecalFunction<'db> {
        DecalFunction::Free(self)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).declaration(db).name(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).declaration(db).span(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).declaration(db).docs(db)
    }

    pub fn params(self, db: &'db dyn Db) -> impl Iterator<Item = DecalParam<'db>> + 'db {
        self.shim(db)
            .params(db)
            .iter()
            .copied()
            .map(|param| DecalParam::new(db, param))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<DecalType<'db>> {
        self.shim(db)
            .ret_type(db)
            .map(|type_| type_.lower_decals(db))
    }

    fn src(self, db: &'db dyn Db) -> Callable<'db> {
        self.shim(db).src(db)
    }

    fn referenced_classes(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.params(db)
            .map(|param| param.type_(db))
            .chain(self.ret_type(db))
            .filter_map(|type_| match type_ {
                DecalType::Class { inner, .. } => Some(inner),
                _ => None,
            })
    }
}

#[salsa::interned(debug)]
pub struct DecalMethod<'db> {
    shim: ShimFunctionAssociated<'db>,
}

#[salsa::tracked]
impl<'db> DecalMethod<'db> {
    pub fn as_fn(self) -> DecalFunction<'db> {
        DecalFunction::Method(self)
    }

    pub fn is_static(self, db: &'db dyn Db) -> bool {
        self.src(db).declaration(db).receiver(db).is_none()
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).declaration(db).name(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).declaration(db).span(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).declaration(db).docs(db)
    }

    pub fn params(self, db: &'db dyn Db) -> impl Iterator<Item = DecalParam<'db>> + 'db {
        self.shim(db)
            .params_without_self(db)
            .iter()
            .copied()
            .map(|param| DecalParam::new(db, param))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<DecalType<'db>> {
        self.shim(db)
            .ret_type(db)
            .map(|type_| type_.lower_decals(db))
    }

    fn src(self, db: &'db dyn Db) -> Callable<'db> {
        self.shim(db).src(db)
    }

    fn referenced_classes(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.params(db)
            .map(|param| param.type_(db))
            .chain(self.ret_type(db))
            .filter_map(DecalType::class)
    }
}

#[salsa::interned(debug)]
pub struct DecalParam<'db> {
    shim: ShimParam<'db>,
}

impl<'db> DecalParam<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        let shim = self.shim(db);
        let (function, param_idx) = match shim.parent(db) {
            ShimFunction::Free(function) => (function.src(db), shim.idx(db)),
            ShimFunction::Associated(function) => {
                let receiver_offset =
                    usize::from(function.src(db).declaration(db).receiver(db).is_some());
                (function.src(db), shim.idx(db) - receiver_offset)
            }
            _ => unreachable!("declaration parameters only belong to source-defined functions"),
        };

        function.params(db)[param_idx].name(db)
    }

    pub fn type_(self, db: &'db dyn Db) -> DecalType<'db> {
        self.shim(db).type_(db).lower_decals(db).try_as_coerced(db)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum DecalClass<'db> {
    Plain(DecalClassPlain<'db>),
    Discriminated(DecalClassDiscriminated<'db>),
}

#[salsa::tracked]
impl<'db> DecalClass<'db> {
    pub fn module(self, db: &'db dyn Db) -> DecalParentModule<'db> {
        match self {
            DecalClass::Plain(plain) => plain.module(db),
            DecalClass::Discriminated(discriminated) => discriminated.module(db),
        }
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self {
            DecalClass::Plain(plain) => plain.name(db),
            DecalClass::Discriminated(discriminated) => discriminated.name(db),
        }
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        match self {
            DecalClass::Plain(plain) => plain.docs(db),
            DecalClass::Discriminated(discriminated) => discriminated.docs(db),
        }
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        match self {
            DecalClass::Plain(plain) => plain.span(db),
            DecalClass::Discriminated(discriminated) => discriminated.span(db),
        }
    }

    pub fn methods(self, db: &'db dyn Db) -> impl Iterator<Item = DecalMethod<'db>> + 'db {
        match self {
            DecalClass::Plain(plain) => Either::Left(plain.methods(db)),
            DecalClass::Discriminated(discriminated) => Either::Right(discriminated.methods(db)),
        }
    }

    pub fn constructor(self, db: &'db dyn Db) -> Option<DecalMethod<'db>> {
        match self {
            DecalClass::Plain(plain) => plain.constructor(db),
            DecalClass::Discriminated(_) => None,
        }
    }

    pub fn coerce(self, db: &'db dyn Db) -> Option<DecalCoerceFunction<'db>> {
        match self {
            DecalClass::Plain(plain) => plain.coerce(db),
            DecalClass::Discriminated(discriminated) => discriminated.coerce(db),
        }
    }

    fn referenced_classes(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        match self {
            DecalClass::Plain(plain) => Either::Left(plain.referenced_classes(db)),
            DecalClass::Discriminated(discriminated) => {
                Either::Right(discriminated.referenced_classes(db))
            }
        }
    }
}

#[salsa::interned(debug)]
pub struct DecalClassPlain<'db> {
    shim: ShimClassPlain<'db>,
}

impl<'db> DecalClassPlain<'db> {
    pub fn module(self, db: &'db dyn Db) -> DecalParentModule<'db> {
        self.shim(db).module(db).decals(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.shim(db).name(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.shim(db).src(db).def(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.shim(db).src(db).def(db).span(db)
    }

    pub fn methods(self, db: &'db dyn Db) -> impl Iterator<Item = DecalMethod<'db>> + 'db {
        self.shim(db)
            .methods(db)
            .iter()
            .map(|function| function.decals(db))
    }

    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = DecalField<'db>> + 'db {
        self.shim(db)
            .fields(db)
            .iter()
            .map(|field| DecalField::new(db, *field))
    }

    pub fn constructor(self, db: &'db dyn Db) -> Option<DecalMethod<'db>> {
        self.methods(db).find(|method| {
            method.name(db) == "new"
                && method.is_static(db)
                && matches!(
                    method.shim(db).ret_type(db),
                    Some(ShimType::Class {
                        is_borrowed: false,
                        inner,
                    }) if inner == self.shim(db).into()
                )
        })
    }

    pub fn coerce(self, db: &'db dyn Db) -> Option<DecalCoerceFunction<'db>> {
        self.shim(db)
            .facade(db)
            .coerce(db)
            .map(|_| DecalCoerceFunction::new(db, DecalClass::Plain(self)))
    }

    fn referenced_classes(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.fields(db)
            .filter_map(|field| field.get_type(db).class())
            .chain(
                self.methods(db)
                    .flat_map(|method| method.referenced_classes(db)),
            )
    }
}

#[salsa::interned(debug)]
pub struct DecalClassDiscriminated<'db> {
    shim: ShimClassDiscriminated<'db>,
}

#[salsa::tracked]
impl<'db> DecalClassDiscriminated<'db> {
    pub fn module(self, db: &'db dyn Db) -> DecalParentModule<'db> {
        self.shim(db).module(db).decals(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.shim(db).name(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.shim(db).src(db).def(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.shim(db).src(db).def(db).span(db)
    }

    pub fn methods(self, db: &'db dyn Db) -> impl Iterator<Item = DecalMethod<'db>> + 'db {
        self.shim(db)
            .methods(db)
            .iter()
            .map(|function| function.decals(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn variants(self, db: &'db dyn Db) -> Vec<DecalVariant<'db>> {
        let shim = self.shim(db);
        shim.variants(db)
            .iter()
            .zip(shim.discriminator(db).facade(db).variant_names(db))
            .map(|(variant, name)| DecalVariant::new(db, *variant, name.clone()))
            .collect()
    }

    pub fn discriminator(self, db: &'db dyn Db) -> DecalDiscriminator<'db> {
        DecalDiscriminator::new(db, self.shim(db).discriminator(db))
    }

    pub fn coerce(self, db: &'db dyn Db) -> Option<DecalCoerceFunction<'db>> {
        self.shim(db)
            .facade(db)
            .coerce(db)
            .map(|_| DecalCoerceFunction::new(db, DecalClass::Discriminated(self)))
    }

    fn referenced_classes(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.variants(db)
            .iter()
            .flat_map(|variant| variant.fields(db))
            .filter_map(|field| field.get_type(db).class())
            .chain(
                self.methods(db)
                    .flat_map(|method| method.referenced_classes(db)),
            )
    }
}

#[salsa::tracked(debug)]
pub struct DecalVariant<'db> {
    shim: ShimClassDiscriminatedVariant<'db>,
    #[returns(ref)]
    #[tracked]
    pub name: String,
}

impl<'db> DecalVariant<'db> {
    pub fn if_coercible(self, db: &'db dyn Db) -> bool {
        self.shim(db).coerce(db).is_some()
    }

    pub fn source_name(self, db: &'db dyn Db) -> &'db String {
        self.shim(db).name(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.shim(db).src(db).declaration(db).span(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.shim(db).src(db).declaration(db).docs(db)
    }

    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = DecalField<'db>> + 'db {
        self.shim(db)
            .fields(db)
            .map(|field| DecalField::new(db, field))
    }
}

#[salsa::interned(debug)]
pub struct DecalField<'db> {
    shim: ShimField<'db>,
}

impl<'db> DecalField<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db FieldName {
        self.shim(db).src(db).declaration(db).name(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.shim(db).src(db).declaration(db).span(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.shim(db).src(db).declaration(db).docs(db)
    }

    pub fn is_readonly(self, db: &'db dyn Db) -> bool {
        self.shim(db).setter(db).is_none()
    }

    pub fn get_type(self, db: &'db dyn Db) -> DecalType<'db> {
        self.shim(db).getter(db).ret_type(db).lower_decals(db)
    }

    pub fn set_type(self, db: &'db dyn Db) -> DecalType<'db> {
        self.get_type(db).try_as_coerced(db)
    }
}

#[salsa::interned(debug)]
pub struct DecalDiscriminator<'db> {
    src: ShimFunctionDiscriminator<'db>,
}

impl<'db> DecalDiscriminator<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.src(db).facade(db).name(db)
    }
}

#[salsa::interned(debug)]
pub struct DecalCoerceFunction<'db> {
    pub class: DecalClass<'db>,
}

impl<'db> DecalCoerceFunction<'db> {
    pub fn input_fields(self, db: &'db dyn Db) -> Vec<DecalField<'db>> {
        match self.class(db) {
            DecalClass::Plain(class) => class.fields(db).collect(),
            DecalClass::Discriminated(class) => class
                .variants(db)
                .iter()
                .filter(|variant| variant.if_coercible(db))
                .flat_map(|variant| variant.fields(db))
                .collect(),
        }
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self.class(db) {
            DecalClass::Plain(c) => c
                .shim(db)
                .facade(db)
                .coerce(db)
                .expect("coerce must exist")
                .name(db),
            DecalClass::Discriminated(c) => c
                .shim(db)
                .facade(db)
                .coerce(db)
                .expect("coerce must exist")
                .name(db),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum DecalType<'db> {
    boolean,
    string,
    number,
    Class {
        inner: DecalClass<'db>,
        coerced: bool,
    },
}

impl<'db> DecalType<'db> {
    pub fn try_as_coerced(self, db: &'db dyn Db) -> Self {
        match self {
            Self::Class { inner, .. } => Self::Class {
                inner,
                coerced: inner.coerce(db).is_some(),
            },
            type_ => type_,
        }
    }

    fn class(self) -> Option<DecalClass<'db>> {
        match self {
            DecalType::Class { inner, .. } => Some(inner),
            _ => None,
        }
    }
}
