//! Intermediate representation used to generate TypeScript declarations.

pub mod ext;
mod lowering;

use std::collections::HashSet;

use itertools::Either;

use crate::compiler::{
    db::Db,
    ir::{
        interface::{
            InterClass, InterClassDiscriminated, InterClassDiscriminatedVariant, InterClassPlain,
            InterClassTemplate, InterField, InterFunctionAssociated, InterFunctionFree,
            InterGeneric, InterMember, InterModule, InterParam, InterParentModule, InterType,
            Interface,
        },
        rust::{Callable, FieldName, Span},
    },
    path::FilePath,
};

/// TypeScript declarations for the reachable interface API.
#[salsa::interned(debug)]
pub struct Decals<'db> {
    interface: Interface<'db>,
}

#[salsa::tracked]
impl<'db> Decals<'db> {
    /// Reachable local modules, sorted by virtual path.
    #[salsa::tracked(returns(ref))]
    pub fn local_modules(self, db: &'db dyn Db) -> Vec<DecalModule<'db>> {
        self.modules(db, true)
    }

    /// Reachable external modules, sorted by virtual path.
    #[salsa::tracked(returns(ref))]
    pub fn external_modules(self, db: &'db dyn Db) -> Vec<DecalModule<'db>> {
        self.modules(db, false)
    }

    fn modules(self, db: &'db dyn Db, is_local: bool) -> Vec<DecalModule<'db>> {
        let mut modules = self
            .interface(db)
            .crates(db)
            .filter(move |crate_| crate_.is_local(db) == is_local)
            .flat_map(move |crate_| crate_.all_modules(db))
            .map(move |module| module.decals(db))
            .filter(|module| {
                module.members(db).any(|member| {
                    matches!(
                        member,
                        DecalMember::Class(_)
                            | DecalMember::ClassTemplate(_)
                            | DecalMember::Function(_)
                    )
                })
            })
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
    interface: InterParentModule<'db>,
}

#[salsa::tracked]
impl<'db> DecalParentModule<'db> {
    pub fn is_local(self, db: &'db dyn Db) -> bool {
        self.interface(db).src(db).path(db).crate_(db).is_local(db)
    }

    pub fn virtual_path(self, db: &'db dyn Db) -> &'db String {
        self.interface(db).facade(db).virtual_path(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.interface(db).src(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.interface(db).src(db).span(db)
    }

    pub fn file_path(self, db: &'db dyn Db) -> &'db FilePath {
        self.interface(db).src(db).file_path(db)
    }
}

/// A reachable TypeScript declaration module.
#[salsa::interned(debug)]
pub struct DecalModule<'db> {
    interface: InterModule<'db>,
}

#[salsa::tracked]
impl<'db> DecalModule<'db> {
    pub fn virtual_path(self, db: &'db dyn Db) -> &'db String {
        self.interface(db).facade(db).virtual_path(db)
    }

    pub fn as_parent(self, db: &'db dyn Db) -> DecalParentModule<'db> {
        self.interface(db).as_parent(db).decals(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.as_parent(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.as_parent(db).span(db)
    }

    /// Members defined in this module.
    pub fn members(self, db: &'db dyn Db) -> impl Iterator<Item = DecalMember<'db>> + 'db {
        self.interface(db)
            .members(db)
            .map(move |member| match member {
                InterMember::Class(class) => DecalMember::Class(class.decals(db)),
                InterMember::Template(template) => {
                    DecalMember::ClassTemplate(DecalClassTemplate::new(db, template))
                }
                InterMember::Function(function) => DecalMember::Function(function.decals(db)),
                InterMember::Module(module) => DecalMember::Module(module.decals(db)),
            })
    }

    /// Imports needed for this module.
    #[salsa::tracked(returns(ref))]
    pub fn imports(self, db: &'db dyn Db) -> Vec<DecalImport<'db>> {
        let mut classes = HashSet::new();

        for member in self.members(db) {
            match member {
                DecalMember::Function(function) => {
                    classes.extend(function.imports(db));
                }
                DecalMember::Class(class) => {
                    classes.extend(class.imports(db));
                }
                DecalMember::ClassTemplate(template) => {
                    classes.extend(template.imports(db));
                }
                DecalMember::Module(_) => {}
            }
        }

        let mut imports = classes
            .into_iter()
            .filter(|class| class.module(db) != self.as_parent(db))
            .map(|class| DecalImport::new(db, self, class))
            .collect::<Vec<_>>();

        imports.sort_by_key(|import| {
            let class = import.class(db);

            (
                class.type_name(db).name(db),
                class.module(db).virtual_path(db),
            )
        });
        imports.dedup_by(|left, right| {
            left.class(db).module(db) == right.class(db).module(db)
                && left.class(db).type_name(db).name(db) == right.class(db).type_name(db).name(db)
        });

        imports
    }

    pub fn file_path(self, db: &'db dyn Db) -> &'db FilePath {
        self.as_parent(db).file_path(db)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum DecalMember<'db> {
    Class(DecalClass<'db>),
    ClassTemplate(DecalClassTemplate<'db>),
    Function(DecalFreeFunction<'db>),
    Module(DecalModule<'db>),
}

#[salsa::interned(debug)]
pub struct DecalClassTemplate<'db> {
    interface: InterClassTemplate<'db>,
}

#[salsa::tracked]
impl<'db> DecalClassTemplate<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.interface(db).name(db)
    }

    pub fn is_discriminated(self, db: &'db dyn Db) -> bool {
        matches!(self.interface(db), InterClassTemplate::Discriminated(_))
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        match self.interface(db) {
            InterClassTemplate::Plain(template) => template.src(db).docs(db),
            InterClassTemplate::Discriminated(template) => template.src(db).docs(db),
        }
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        match self.interface(db) {
            InterClassTemplate::Plain(template) => template.src(db).span(db),
            InterClassTemplate::Discriminated(template) => template.src(db).span(db),
        }
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<String> {
        self.interface(db).param_names(db)
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.mappings(db)
            .iter()
            .flat_map(move |(_, class)| class.imports(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn mappings(self, db: &'db dyn Db) -> Vec<(Vec<DecalGeneric<'db>>, DecalClass<'db>)> {
        self.interface(db)
            .mappings(db)
            .iter()
            .map(|(generics, class)| {
                let generics = generics
                    .iter()
                    .map(|generic| match generic {
                        InterGeneric::Symbol(symbol) => DecalGeneric::Symbol(symbol.clone()),
                        InterGeneric::Class(class) => DecalGeneric::Class(class.decals(db)),
                    })
                    .collect();

                (generics, class.decals(db))
            })
            .collect()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub enum DecalGeneric<'db> {
    Symbol(String),
    Class(DecalClass<'db>),
}

#[salsa::interned(debug)]
pub struct TypeName<'db> {
    #[returns(ref)]
    pub name: String,
    #[returns(ref)]
    pub args: Option<Vec<DecalGeneric<'db>>>,
}

#[salsa::tracked]
impl<'db> TypeName<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn imports(self, db: &'db dyn Db) -> Vec<DecalClass<'db>> {
        self.args(db)
            .iter()
            .flatten()
            .flat_map(move |generic| match generic {
                DecalGeneric::Class(class) => Either::Left(
                    std::iter::once(*class).chain(class.type_name(db).imports(db).iter().copied()),
                ),
                DecalGeneric::Symbol(_) => Either::Right(std::iter::empty()),
            })
            .collect()
    }
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

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.params(db)
            .flat_map(move |param| param.imports(db))
            .chain(
                self.ret_type(db)
                    .into_iter()
                    .flat_map(move |type_| type_.imports(db)),
            )
    }
}

#[salsa::interned(debug)]
pub struct DecalFreeFunction<'db> {
    interface: InterFunctionFree<'db>,
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
        self.interface(db)
            .params(db)
            .iter()
            .copied()
            .map(|param| DecalParam::new(db, param))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<DecalType<'db>> {
        self.interface(db)
            .ret_type(db)
            .map(|type_| type_.lower_decals(db))
    }

    fn src(self, db: &'db dyn Db) -> Callable<'db> {
        self.interface(db).src(db)
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.as_fn().imports(db)
    }
}

#[salsa::interned(debug)]
pub struct DecalMethod<'db> {
    interface: InterFunctionAssociated<'db>,
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
        self.interface(db)
            .params_without_self(db)
            .iter()
            .copied()
            .map(|param| DecalParam::new(db, param))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<DecalType<'db>> {
        self.interface(db)
            .ret_type(db)
            .map(|type_| type_.lower_decals(db))
    }

    fn src(self, db: &'db dyn Db) -> Callable<'db> {
        self.interface(db).src(db)
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.as_fn().imports(db)
    }
}

#[salsa::interned(debug)]
pub struct DecalParam<'db> {
    interface: InterParam<'db>,
}

impl<'db> DecalParam<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.lower_name(db)
    }

    pub fn type_(self, db: &'db dyn Db) -> DecalType<'db> {
        self.interface(db)
            .type_(db)
            .lower_decals(db)
            .try_as_coerced(db)
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.type_(db).imports(db)
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
            Self::Plain(class) => class.name(db),
            Self::Discriminated(class) => class.name(db),
        }
    }

    pub fn is_template_instance(self, db: &'db dyn Db) -> bool {
        match self {
            DecalClass::Plain(class) => class.is_template_instance(db),
            DecalClass::Discriminated(class) => class.is_template_instance(db),
        }
    }

    pub fn type_name(self, db: &'db dyn Db) -> TypeName<'db> {
        let (segment, generics) = match self {
            DecalClass::Plain(class) => (
                class.interface(db).src(db).segment(db),
                InterClass::Plain(class.interface(db)).generics(db),
            ),
            DecalClass::Discriminated(class) => (
                class.interface(db).src(db).segment(db),
                InterClass::Discriminated(class.interface(db)).generics(db),
            ),
        };
        let args = self.is_template_instance(db).then(|| {
            generics
                .map(|generic| match generic {
                    InterGeneric::Class(class) => DecalGeneric::Class(class.decals(db)),
                    InterGeneric::Symbol(symbol) => DecalGeneric::Symbol(symbol),
                })
                .collect()
        });

        TypeName::new(db, segment.name(db).clone(), args)
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

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        match self {
            DecalClass::Plain(plain) => Either::Left(plain.imports(db)),
            DecalClass::Discriminated(discriminated) => Either::Right(discriminated.imports(db)),
        }
    }
}

#[salsa::interned(debug)]
pub struct DecalClassPlain<'db> {
    interface: InterClassPlain<'db>,
}

impl<'db> DecalClassPlain<'db> {
    pub fn module(self, db: &'db dyn Db) -> DecalParentModule<'db> {
        self.interface(db).module(db).decals(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.interface(db).name(db)
    }

    pub fn is_template_instance(self, db: &'db dyn Db) -> bool {
        !self.interface(db).src(db).gen_args(db).is_empty()
    }

    pub fn type_name(self, db: &'db dyn Db) -> TypeName<'db> {
        DecalClass::Plain(self).type_name(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.interface(db).src(db).def(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.interface(db).src(db).def(db).span(db)
    }

    pub fn methods(self, db: &'db dyn Db) -> impl Iterator<Item = DecalMethod<'db>> + 'db {
        self.interface(db)
            .methods(db)
            .iter()
            .map(|function| function.decals(db))
    }

    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = DecalField<'db>> + 'db {
        self.interface(db)
            .fields(db)
            .iter()
            .map(|field| DecalField::new(db, *field))
    }

    pub fn constructor(self, db: &'db dyn Db) -> Option<DecalMethod<'db>> {
        self.methods(db).find(|method| {
            method.name(db) == "new"
                && method.is_static(db)
                && matches!(
                    method.interface(db).ret_type(db),
                    Some(InterType::Class {
                        is_borrowed: false,
                        inner,
                    }) if inner
                        .src_type(db)
                        .same_type_as(db, self.interface(db).src(db).into_type(db))
                )
        })
    }

    pub fn coerce(self, db: &'db dyn Db) -> Option<DecalCoerceFunction<'db>> {
        self.interface(db)
            .facade(db)
            .coerce(db)
            .map(|_| DecalCoerceFunction::new(db, DecalClass::Plain(self)))
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.fields(db)
            .flat_map(move |field| field.imports(db))
            .chain(self.methods(db).flat_map(move |method| method.imports(db)))
            .chain(self.type_name(db).imports(db).iter().copied())
    }
}

#[salsa::interned(debug)]
pub struct DecalClassDiscriminated<'db> {
    interface: InterClassDiscriminated<'db>,
}

#[salsa::tracked]
impl<'db> DecalClassDiscriminated<'db> {
    pub fn module(self, db: &'db dyn Db) -> DecalParentModule<'db> {
        self.interface(db).module(db).decals(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.interface(db).name(db)
    }

    pub fn is_template_instance(self, db: &'db dyn Db) -> bool {
        !self.interface(db).src(db).gen_args(db).is_empty()
    }

    pub fn type_name(self, db: &'db dyn Db) -> TypeName<'db> {
        DecalClass::Discriminated(self).type_name(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.interface(db).src(db).def(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.interface(db).src(db).def(db).span(db)
    }

    pub fn methods(self, db: &'db dyn Db) -> impl Iterator<Item = DecalMethod<'db>> + 'db {
        self.interface(db)
            .methods(db)
            .iter()
            .map(|function| function.decals(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn variants(self, db: &'db dyn Db) -> Vec<DecalVariant<'db>> {
        let interface = self.interface(db);
        interface
            .variants(db)
            .iter()
            .zip(interface.facade(db).discriminator(db).variant_names(db))
            .map(|(variant, name)| DecalVariant::new(db, *variant, name.clone()))
            .collect()
    }

    pub fn discriminator(self, db: &'db dyn Db) -> DecalDiscriminator<'db> {
        DecalDiscriminator::new(db, self.interface(db))
    }

    pub fn coerce(self, db: &'db dyn Db) -> Option<DecalCoerceFunction<'db>> {
        self.interface(db)
            .facade(db)
            .coerce(db)
            .map(|_| DecalCoerceFunction::new(db, DecalClass::Discriminated(self)))
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.variants(db)
            .iter()
            .flat_map(move |variant| variant.imports(db))
            .chain(self.methods(db).flat_map(move |method| method.imports(db)))
            .chain(self.type_name(db).imports(db).iter().copied())
    }
}

#[salsa::tracked(debug)]
pub struct DecalVariant<'db> {
    interface: InterClassDiscriminatedVariant<'db>,
    #[returns(ref)]
    #[tracked]
    pub name: String,
}

impl<'db> DecalVariant<'db> {
    pub fn if_coercible(self, db: &'db dyn Db) -> bool {
        self.interface(db).is_constructible(db)
    }

    pub fn source_name(self, db: &'db dyn Db) -> &'db String {
        self.interface(db).name(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.interface(db).src(db).declaration(db).span(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.interface(db).src(db).declaration(db).docs(db)
    }

    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = DecalField<'db>> + 'db {
        self.interface(db)
            .fields(db)
            .map(|field| DecalField::new(db, field))
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.fields(db).flat_map(move |field| field.imports(db))
    }
}

#[salsa::interned(debug)]
pub struct DecalField<'db> {
    interface: InterField<'db>,
}

impl<'db> DecalField<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db FieldName {
        self.interface(db).src(db).declaration(db).name(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.interface(db).src(db).declaration(db).span(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.interface(db).src(db).declaration(db).docs(db)
    }

    pub fn is_readonly(self, db: &'db dyn Db) -> bool {
        self.interface(db).is_readonly(db)
    }

    pub fn get_type(self, db: &'db dyn Db) -> DecalType<'db> {
        self.interface(db).type_(db).lower_decals(db)
    }

    pub fn set_type(self, db: &'db dyn Db) -> DecalType<'db> {
        self.get_type(db).try_as_coerced(db)
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        self.get_type(db).imports(db)
    }
}

#[salsa::interned(debug)]
pub struct DecalDiscriminator<'db> {
    src: InterClassDiscriminated<'db>,
}

impl<'db> DecalDiscriminator<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.src(db).facade(db).discriminator(db).name(db)
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
                .interface(db)
                .facade(db)
                .coerce(db)
                .expect("coerce must exist")
                .name(db),
            DecalClass::Discriminated(c) => c
                .interface(db)
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

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = DecalClass<'db>> + 'db {
        match self {
            Self::Class { inner, .. } => Either::Left(
                std::iter::once(inner).chain(inner.type_name(db).imports(db).iter().copied()),
            ),
            _ => Either::Right(std::iter::empty()),
        }
    }
}
