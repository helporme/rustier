//! Intermediate representation used to generate the JavaScript API.

pub mod ext;
mod lowering;

use std::collections::{BTreeMap, HashMap, HashSet};

use itertools::{Either, Itertools};

use crate::compiler::{
    codegen::{
        fragment::{FragmentPrint, frag},
        utils::to_camel_case,
    },
    db::Db,
    ir::{
        ffi::{Ffi, FfiClass, FfiFunction},
        rust::{Callable, FieldName},
        shim::{
            Shim, ShimClassDiscriminated, ShimClassPlain, ShimClassTemplate, ShimField,
            ShimFunctionAssociated, ShimFunctionDiscriminator, ShimFunctionFree,
            ShimFunctionPlainCoerce, ShimGeneric, ShimMember, ShimModule, ShimParam,
            ShimParentModule, ShimType,
        },
    },
    path::FilePath,
};

/// The facade for a complete reachable shim API.
#[salsa::interned(debug)]
pub struct Facade<'db> {
    shim: Shim<'db>,
}

impl<'db> Facade<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> Ffi<'db> {
        Ffi::new(db, self.shim(db))
    }
}

/// An import of a facade item.
///
/// Matching the js `import`/`export` statement.
#[salsa::tracked(debug)]
pub struct FacadeImport<'db> {
    /// The module where the import statement will be emitted.
    pub originating_from: FacadeModule<'db>,
    /// Imported item.
    pub item: FacadeImportItem<'db>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeImportItem<'db> {
    /// Import of an FFI function.
    FfiFunction(FfiFunction<'db>),
    /// Import of an FFI class.
    FfiClass(FfiClass<'db>),
    /// Import of a facade class.
    FacadeClass(FacadeClass<'db>),
}

impl<'db> FacadeImportItem<'db> {
    /// The symbol of the import item.
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self {
            FacadeImportItem::FfiFunction(f) => f.symbol(db),
            FacadeImportItem::FfiClass(c) => c.symbol(db),
            FacadeImportItem::FacadeClass(c) => c.name(db),
        }
    }
}

/// A facade module used as a parent of another facade entity.
///
/// Matches a single `.js` file generated for a rust module. Take into accout
/// that there is no crate containing this module since crates aren't a thing in
/// js.
#[salsa::interned(debug)]
pub struct FacadeParentModule<'db> {
    shim: ShimParentModule<'db>,
}

#[salsa::tracked]
impl<'db> FacadeParentModule<'db> {
    pub fn is_local(self, db: &'db dyn Db) -> bool {
        self.shim(db).src(db).path(db).crate_(db).is_local(db)
    }

    /// Stable module specifier independent of source file layout.
    #[salsa::tracked(returns(ref))]
    pub fn virtual_path(self, db: &'db dyn Db) -> String {
        let path = self.shim(db).src(db).path(db);
        let names = path
            .segments(db)
            .iter()
            .map(|segment| segment.name(db).as_str())
            .collect::<Vec<_>>();

        format!("$rust/{}", names.join("/"))
    }

    /// File path of the source rust module.
    pub fn file_path(self, db: &'db dyn Db) -> &'db FilePath {
        self.shim(db).src(db).file_path(db)
    }
}

/// A reachable facade module.
#[salsa::interned(debug)]
pub struct FacadeModule<'db> {
    shim: ShimModule<'db>,
}

#[salsa::tracked]
impl<'db> FacadeModule<'db> {
    pub fn virtual_path(self, db: &'db dyn Db) -> &'db String {
        self.as_parent(db).virtual_path(db)
    }

    pub fn as_parent(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        self.shim(db).as_parent(db).facade(db)
    }

    /// All members defined in this module.
    pub fn members(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeMember<'db>> + 'db {
        self.shim(db).members(db).map(move |member| match member {
            ShimMember::Class(class) => FacadeMember::Class(class.facade(db)),
            ShimMember::Template(template) => {
                FacadeMember::ClassFactory(FacadeClassFactory::new(db, template))
            }
            ShimMember::Function(function) => FacadeMember::Function(function.facade(db)),
            ShimMember::Module(module) => FacadeMember::Module(module.facade(db)),
        })
    }

    /// Imports needed for this module.
    #[salsa::tracked(returns(ref))]
    pub fn imports(self, db: &'db dyn Db) -> Vec<FacadeImport<'db>> {
        let mut items = Vec::new();

        for member in self.members(db) {
            match member {
                FacadeMember::Function(function) => {
                    items.extend(function.imports(db).iter().copied());
                }
                FacadeMember::Class(class) => {
                    items.extend(class.imports(db));
                }
                FacadeMember::ClassFactory(factory) => {
                    items.extend(factory.imports(db));
                }
                FacadeMember::Module(_) => {}
            }
        }

        items
            .into_iter()
            .filter(|item| {
                !matches!(
                    item,
                    FacadeImportItem::FacadeClass(class) if class.module(db) == self.as_parent(db)
                )
            })
            .unique_by(|item| {
                let module = match item {
                    FacadeImportItem::FacadeClass(class) => Some(class.module(db)),
                    FacadeImportItem::FfiClass(_) | FacadeImportItem::FfiFunction(_) => None,
                };

                (module, item.name(db))
            })
            .map(|item| FacadeImport::new(db, self, item))
            .collect()
    }

    /// File path of the source rust module.
    pub fn file_path(self, db: &'db dyn Db) -> &'db FilePath {
        self.as_parent(db).file_path(db)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeMember<'db> {
    Class(FacadeClass<'db>),
    ClassFactory(FacadeClassFactory<'db>),
    Function(FacadeFreeFunction<'db>),
    Module(FacadeModule<'db>),
}

#[salsa::interned(debug)]
pub struct FacadeClassFactory<'db> {
    shim: ShimClassTemplate<'db>,
}

#[salsa::tracked]
impl<'db> FacadeClassFactory<'db> {
    pub fn module(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        self.shim(db).module(db).facade(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.shim(db).name(db)
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> + 'db {
        self.mappings(db).iter().flat_map(move |(generics, class)| {
            generics
                .iter()
                .filter_map(|generic| match generic {
                    FacadeGeneric::Class(class) => Some(FacadeImportItem::FacadeClass(*class)),
                    FacadeGeneric::Symbol(_) => None,
                })
                .chain(class.imports(db))
        })
    }

    #[salsa::tracked(returns(ref))]
    pub fn mappings(self, db: &'db dyn Db) -> Vec<(Vec<FacadeGeneric<'db>>, FacadeClass<'db>)> {
        self.shim(db)
            .mappings(db)
            .iter()
            .map(|(generics, class)| {
                let generics = generics
                    .iter()
                    .map(|generic| match generic {
                        ShimGeneric::Symbol(symbol) => FacadeGeneric::Symbol(symbol.clone()),
                        ShimGeneric::Class(class) => FacadeGeneric::Class(class.facade(db)),
                    })
                    .collect();

                (generics, class.facade(db))
            })
            .collect()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub enum FacadeGeneric<'db> {
    Symbol(String),
    Class(FacadeClass<'db>),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeFunction<'db> {
    Free(FacadeFreeFunction<'db>),
    Method(FacadeMethod<'db>),
}

#[salsa::tracked]
impl<'db> FacadeFunction<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        match self {
            Self::Free(function) => function.name(db),
            Self::Method(method) => method.name(db),
        }
    }

    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        match self {
            Self::Free(function) => function.ffi(db),
            Self::Method(method) => method.ffi(db),
        }
    }

    pub fn params(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeParam<'db>> + 'db {
        match self {
            Self::Free(function) => Either::Left(function.params(db)),
            Self::Method(method) => Either::Right(method.params(db)),
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<FacadeType<'db>> {
        match self {
            Self::Free(function) => function.ret_type(db),
            Self::Method(method) => method.ret_type(db),
        }
    }
}

#[salsa::interned(debug)]
pub struct FacadeFreeFunction<'db> {
    shim: ShimFunctionFree<'db>,
}

#[salsa::tracked]
impl<'db> FacadeFreeFunction<'db> {
    pub fn as_fn(self) -> FacadeFunction<'db> {
        FacadeFunction::Free(self)
    }

    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).ffi(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).declaration(db).name(db)
    }

    pub fn params(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeParam<'db>> + 'db {
        self.shim(db)
            .params(db)
            .iter()
            .copied()
            .map(|param| FacadeParam::new(db, param))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<FacadeType<'db>> {
        self.shim(db)
            .ret_type(db)
            .map(|type_| type_.lower_facade(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn imports(self, db: &'db dyn Db) -> Vec<FacadeImportItem<'db>> {
        let mut imports = HashSet::from([FacadeImportItem::FfiFunction(self.ffi(db))]);

        if let Some(FacadeType::Class(class)) = self.ret_type(db) {
            imports.insert(FacadeImportItem::FfiClass(class.ffi(db)));
        }

        for type_ in self
            .params(db)
            .map(|param| param.type_(db))
            .chain(self.ret_type(db))
        {
            if let FacadeType::Class(class) = type_ {
                imports.insert(FacadeImportItem::FacadeClass(class));
            }
        }

        imports.into_iter().collect()
    }

    fn src(self, db: &'db dyn Db) -> Callable<'db> {
        self.shim(db).src(db)
    }
}

#[salsa::interned(debug)]
pub struct FacadeMethod<'db> {
    shim: ShimFunctionAssociated<'db>,
}

#[salsa::tracked]
impl<'db> FacadeMethod<'db> {
    pub fn as_fn(self) -> FacadeFunction<'db> {
        FacadeFunction::Method(self)
    }

    pub fn is_static(self, db: &'db dyn Db) -> bool {
        self.src(db).declaration(db).receiver(db).is_none()
    }

    pub fn class(self, db: &'db dyn Db) -> FacadeClass<'db> {
        self.shim(db).self_class(db).facade(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).declaration(db).name(db)
    }

    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).ffi(db)
    }

    pub fn params(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeParam<'db>> + 'db {
        self.shim(db)
            .params_without_self(db)
            .iter()
            .copied()
            .map(|param| FacadeParam::new(db, param))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<FacadeType<'db>> {
        self.shim(db)
            .ret_type(db)
            .map(|type_| type_.lower_facade(db))
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> + 'db {
        self.params(db)
            .map(|param| param.type_(db))
            .chain(self.ret_type(db))
            .filter_map(|cls| match cls {
                FacadeType::Class(cls) => Some(cls),
                _ => None,
            })
            .map(FacadeImportItem::FacadeClass)
            .chain(std::iter::once(FacadeImportItem::FfiFunction(self.ffi(db))))
            .chain(self.ret_type(db).and_then(|type_| match type_ {
                FacadeType::Class(class) => Some(FacadeImportItem::FfiClass(class.ffi(db))),
                _ => None,
            }))
    }

    fn src(self, db: &'db dyn Db) -> Callable<'db> {
        self.shim(db).src(db)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeCoerce<'db> {
    Plain(FacadePlainCoerceFunction<'db>),
    Discriminated(FacadeDiscriminatedCoerceFunction<'db>),
}

impl<'db> FacadeCoerce<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self {
            Self::Plain(coerce) => coerce.name(db),
            Self::Discriminated(coerce) => coerce.name(db),
        }
    }

    pub fn class(self, db: &'db dyn Db) -> FacadeClass<'db> {
        match self {
            Self::Plain(coerce) => FacadeClass::Plain(coerce.class(db)),
            Self::Discriminated(coerce) => FacadeClass::Discriminated(coerce.class(db)),
        }
    }
}

#[salsa::interned(debug)]
pub struct FacadePlainCoerceFunction<'db> {
    shim: ShimFunctionPlainCoerce<'db>,
}

impl<'db> FacadePlainCoerceFunction<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        if self
            .class(db)
            .methods(db)
            .any(|method| method.is_static(db) && method.name(db) == "of")
        {
            "$of"
        } else {
            "of"
        }
    }

    pub fn class(self, db: &'db dyn Db) -> FacadeClassPlain<'db> {
        self.shim(db).class(db).facade(db)
    }

    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).ffi(db)
    }

    pub fn imports(self, db: &'db dyn Db) -> Vec<FacadeImportItem<'db>> {
        let class = FacadeClass::Plain(self.class(db));
        [
            FacadeImportItem::FfiFunction(self.ffi(db)),
            FacadeImportItem::FfiClass(class.ffi(db)),
            FacadeImportItem::FacadeClass(class),
        ]
        .into_iter()
        .chain(
            self.fields(db)
                .filter_map(|(_, param)| match param.type_(db) {
                    FacadeType::Class(class) => Some(FacadeImportItem::FacadeClass(class)),
                    _ => None,
                }),
        )
        .collect()
    }

    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = (FieldName, FacadeParam<'db>)> {
        self.shim(db)
            .field_param_coerce(db)
            .iter()
            .map(|(name, param)| (name.clone(), FacadeParam::new(db, *param)))
    }
}

#[salsa::interned(debug)]
pub struct FacadeDiscriminatedCoerceFunction<'db> {
    shim: ShimClassDiscriminated<'db>,
}

#[salsa::tracked]
impl<'db> FacadeDiscriminatedCoerceFunction<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        if self
            .class(db)
            .methods(db)
            .any(|method| method.is_static(db) && method.name(db) == "of")
        {
            "$of"
        } else {
            "of"
        }
    }

    pub fn class(self, db: &'db dyn Db) -> FacadeClassDiscriminated<'db> {
        self.shim(db).facade(db)
    }

    pub fn imports(self, db: &'db dyn Db) -> Vec<FacadeImportItem<'db>> {
        let class = FacadeClass::Discriminated(self.class(db));
        self.coerce_variants(db)
            .iter()
            .flat_map(|variant| {
                [
                    FacadeImportItem::FfiFunction(variant.ffi),
                    FacadeImportItem::FfiClass(class.ffi(db)),
                    FacadeImportItem::FacadeClass(class),
                ]
                .into_iter()
                .chain(
                    variant
                        .fields
                        .iter()
                        .filter_map(|(_, param)| match param.type_(db) {
                            FacadeType::Class(class) => Some(FacadeImportItem::FacadeClass(class)),
                            _ => None,
                        }),
                )
            })
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub fn coerce_variants(self, db: &'db dyn Db) -> Vec<CoerceVariant<'db>> {
        self.shim(db)
            .variants(db)
            .iter()
            .zip(self.class(db).discriminator(db).variant_names(db))
            .filter_map(|(v, name)| {
                v.coerce(db).map(|coerce| CoerceVariant {
                    name: name.clone(),
                    ffi: coerce.ffi(db),
                    fields: coerce
                        .field_to_params(db)
                        .iter()
                        .map(|(name, param)| (name.clone(), FacadeParam::new(db, *param)))
                        .collect(),
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub struct CoerceVariant<'db> {
    pub name: String,
    pub ffi: FfiFunction<'db>,
    pub fields: Vec<(FieldName, FacadeParam<'db>)>,
}

#[salsa::interned(debug)]
pub struct FacadeParam<'db> {
    shim: ShimParam<'db>,
}

impl<'db> FacadeParam<'db> {
    pub fn type_(self, db: &'db dyn Db) -> FacadeType<'db> {
        self.shim(db).type_(db).lower_facade(db)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeClass<'db> {
    Plain(FacadeClassPlain<'db>),
    Discriminated(FacadeClassDiscriminated<'db>),
}

#[salsa::tracked]
impl<'db> FacadeClass<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        match self {
            FacadeClass::Plain(plain) => plain.ffi(db),
            FacadeClass::Discriminated(discriminated) => discriminated.ffi(db),
        }
    }

    pub fn module(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        match self {
            FacadeClass::Plain(plain) => plain.module(db),
            FacadeClass::Discriminated(discriminated) => discriminated.module(db),
        }
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self {
            FacadeClass::Plain(plain) => plain.name(db),
            FacadeClass::Discriminated(discriminated) => discriminated.name(db),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn import_alias(self, db: &'db dyn Db) -> String {
        frag!({"$facade"}{self.ffi(db).symbol(db)}).print()
    }

    pub fn methods(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeMethod<'db>> + 'db {
        match self {
            FacadeClass::Plain(plain) => Either::Left(plain.methods(db)),
            FacadeClass::Discriminated(discriminated) => Either::Right(discriminated.methods(db)),
        }
    }

    pub fn constructor(self, db: &'db dyn Db) -> Option<FacadeMethod<'db>> {
        match self {
            FacadeClass::Plain(plain) => plain.constructor(db),
            FacadeClass::Discriminated(_) => None,
        }
    }

    pub fn coerce(self, db: &'db dyn Db) -> Option<FacadeCoerce<'db>> {
        match self {
            FacadeClass::Plain(plain) => plain.coerce(db).map(FacadeCoerce::Plain),
            FacadeClass::Discriminated(discriminated) => {
                discriminated.coerce(db).map(FacadeCoerce::Discriminated)
            }
        }
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> + 'db {
        match self {
            FacadeClass::Plain(plain) => Either::Left(plain.imports(db)),
            FacadeClass::Discriminated(discriminated) => Either::Right(discriminated.imports(db)),
        }
    }
}

#[salsa::interned(debug)]
pub struct FacadeClassPlain<'db> {
    shim: ShimClassPlain<'db>,
}

impl<'db> FacadeClassPlain<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        self.shim(db).ffi(db)
    }

    pub fn module(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        self.shim(db).module(db).facade(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.shim(db).name(db)
    }

    pub fn methods(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeMethod<'db>> + 'db {
        self.shim(db)
            .methods(db)
            .iter()
            .map(|function| function.facade(db))
    }

    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeFieldPlain<'db>> + 'db {
        self.shim(db)
            .fields(db)
            .iter()
            .map(|field| FacadeFieldPlain::new(db, *field))
    }

    pub fn constructor(self, db: &'db dyn Db) -> Option<FacadeMethod<'db>> {
        self.methods(db).find(|method| {
            method.name(db) == "new"
                && method.is_static(db)
                && matches!(
                    method.shim(db).ret_type(db),
                    Some(ShimType::Class {
                        is_borrowed: false,
                        inner,
                    }) if inner.symbol(db) == self.shim(db).symbol(db)
                )
        })
    }

    pub fn coerce(self, db: &'db dyn Db) -> Option<FacadePlainCoerceFunction<'db>> {
        self.shim(db)
            .coerce(db)
            .map(|fn_| FacadePlainCoerceFunction::new(db, fn_))
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> + 'db {
        self.fields(db)
            .flat_map(|f| f.imports(db))
            .chain(self.methods(db).flat_map(|m| m.imports(db)))
            .chain(
                self.coerce(db)
                    .into_iter()
                    .flat_map(move |coerce| coerce.imports(db)),
            )
            .chain(std::iter::once(FacadeImportItem::FfiClass(self.ffi(db))))
    }
}

#[salsa::interned(debug)]
pub struct FacadeClassDiscriminated<'db> {
    shim: ShimClassDiscriminated<'db>,
}

#[salsa::tracked]
impl<'db> FacadeClassDiscriminated<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        self.shim(db).ffi(db)
    }

    pub fn module(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        self.shim(db).module(db).facade(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.shim(db).name(db)
    }

    pub fn methods(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeMethod<'db>> + 'db {
        self.shim(db)
            .methods(db)
            .iter()
            .map(|function| function.facade(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<FacadeFieldDiscriminated<'db>> {
        self.shim(db)
            .variants(db)
            .iter()
            .copied()
            .flat_map(|variant| variant.fields(db).map(move |field| (field, variant)))
            .into_group_map_by(|(field, _)| field.name(db))
            .into_iter()
            .sorted_by_key(|(name, _)| name.to_string())
            .map(|(name, pairs)| {
                FacadeFieldDiscriminated::new(
                    db,
                    self,
                    name.clone(),
                    pairs
                        .into_iter()
                        .map(|(field, variant)| {
                            (variant.tag_value(db), FacadeFieldPlain::new(db, field))
                        })
                        .collect(),
                )
            })
            .collect()
    }

    pub fn discriminator(self, db: &'db dyn Db) -> FacadeFieldDiscriminator<'db> {
        FacadeFieldDiscriminator::new(db, self.shim(db).discriminator(db))
    }

    pub fn coerce(self, db: &'db dyn Db) -> Option<FacadeDiscriminatedCoerceFunction<'db>> {
        let shim = self.shim(db);
        shim.is_coercable(db)
            .then(|| FacadeDiscriminatedCoerceFunction::new(db, shim))
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> + 'db {
        self.fields(db)
            .iter()
            .flat_map(|f| f.imports(db))
            .chain(self.methods(db).flat_map(|m| m.imports(db)))
            .chain(
                self.coerce(db)
                    .into_iter()
                    .flat_map(move |coerce| coerce.imports(db)),
            )
            .chain(self.discriminator(db).imports(db))
    }
}

#[salsa::interned(debug)]
pub struct FacadeFieldPlain<'db> {
    shim: ShimField<'db>,
}

impl<'db> FacadeFieldPlain<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db FieldName {
        self.shim(db).src(db).declaration(db).name(db)
    }

    pub fn ffi_setter(self, db: &'db dyn Db) -> Option<FfiFunction<'db>> {
        self.shim(db).setter(db).map(|f| f.ffi(db))
    }

    pub fn ffi_getter(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).getter(db).ffi(db)
    }

    pub fn type_(self, db: &'db dyn Db) -> FacadeType<'db> {
        self.shim(db).getter(db).ret_type(db).lower_facade(db)
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> {
        [
            Some(FacadeImportItem::FfiFunction(self.ffi_getter(db))),
            self.ffi_setter(db).map(FacadeImportItem::FfiFunction),
            match self.type_(db) {
                FacadeType::Class(class) => Some(FacadeImportItem::FacadeClass(class)),
                _ => None,
            },
            match self.type_(db) {
                FacadeType::Class(class) => Some(FacadeImportItem::FfiClass(class.ffi(db))),
                _ => None,
            },
        ]
        .into_iter()
        .flatten()
    }
}

/// A field which value depends on the discriminator tag value.
#[salsa::tracked(debug)]
pub struct FacadeFieldDiscriminated<'db> {
    pub class: FacadeClassDiscriminated<'db>,
    #[returns(ref)]
    pub name: FieldName,
    /// A map from discriminator tag value to the field.
    #[returns(ref)]
    #[tracked]
    pub sub: BTreeMap<u32, FacadeFieldPlain<'db>>,
}

impl<'db> FacadeFieldDiscriminated<'db> {
    pub fn imports(&self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> {
        self.sub(db).values().flat_map(|f| f.imports(db))
    }
}

/// Discriminator field of a [`FacadeClassDiscriminated`].
#[salsa::interned(debug)]
pub struct FacadeFieldDiscriminator<'db> {
    shim: ShimFunctionDiscriminator<'db>,
}

#[salsa::tracked]
impl<'db> FacadeFieldDiscriminator<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'static str {
        let enum_ = self.shim(db).src(db);

        if enum_.variants(db).any(|variant| {
            variant
                .fields(db)
                .any(|field| matches!(field.declaration(db).name(db), FieldName::Name(name) if name == "type"))
        }) {
            "$type"
        } else {
            "type"
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn variant_names(self, db: &'db dyn Db) -> Vec<String> {
        let shim = self.shim(db).class(db);

        let camel_case_names = shim
            .variants(db)
            .iter()
            .map(|variant| to_camel_case(variant.name(db)))
            .collect::<Vec<_>>();

        let mut occurrences = HashMap::new();

        for name in &camel_case_names {
            *occurrences.entry(name.clone()).or_insert(0) += 1;
        }

        shim.variants(db)
            .iter()
            .zip(camel_case_names)
            .map(|(variant, camel_case_name)| {
                if occurrences[&camel_case_name] > 1 {
                    variant.name(db).to_owned()
                } else {
                    camel_case_name
                }
            })
            .collect()
    }

    pub fn ffi_getter(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).ffi(db)
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> {
        std::iter::once(FacadeImportItem::FfiFunction(self.ffi_getter(db)))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum FacadeType<'db> {
    boolean,
    string,
    number,
    Class(FacadeClass<'db>),
}
