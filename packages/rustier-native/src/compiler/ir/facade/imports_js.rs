use std::collections::HashSet;

use itertools::{Either, Itertools};

use crate::compiler::{
    db::Db,
    ir::{
        abi::AbiFunction,
        facade::{
            FacadeClass, FacadeClassDiscriminated, FacadeClassPlain, FacadeClassTemplate,
            FacadeFieldDiscriminated, FacadeFieldDiscriminator, FacadeFieldPlain,
            FacadeFreeFunction, FacadeGeneric, FacadeMember, FacadeMethod, FacadeModule,
            coerce::{FacadeCoerce, FacadeDiscriminatedCoerceFunction, FacadePlainCoerceFunction},
            traits::{FacadeTrait, FacadeTraitArgument},
            type_js::FacadeTypeJs,
        },
        runtime::RuntimeExport,
    },
};

#[salsa::tracked(debug)]
pub struct FacadeImportJs<'db> {
    pub originating_from: FacadeModule<'db>,
    pub item: FacadeImportItemJs<'db>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub enum FacadeImportItemJs<'db> {
    AbiFunction(AbiFunction<'db>),
    FacadeClass(FacadeClass<'db>),
    Runtime(RuntimeExport),
}

impl<'db> FacadeImportItemJs<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self {
            Self::AbiFunction(f) => f.symbol(db),
            Self::FacadeClass(c) => c.name(db),
            Self::Runtime(export) => export.name(),
        }
    }
}

#[salsa::tracked]
impl<'db> FacadeModule<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn imports(self, db: &'db dyn Db) -> Vec<FacadeImportJs<'db>> {
        let mut items = Vec::new();

        for member in self.members(db).iter().copied() {
            match member {
                FacadeMember::Function(function) => {
                    items.extend(function.imports(db).iter().copied())
                }
                FacadeMember::Class(class) => items.extend(class.imports(db)),
                FacadeMember::ClassTemplate(template) => items.extend(template.imports(db)),
                FacadeMember::Trait(trait_) => items.extend(trait_.imports(db)),
                FacadeMember::Module(_) => {}
            }
        }

        items.into_iter()
            .filter(|item| !matches!(item, FacadeImportItemJs::FacadeClass(class) if class.module(db) == self.as_parent(db)))
            .unique_by(|item| {
                let module = match item {
                    FacadeImportItemJs::FacadeClass(class) => Some(class.module(db)),
                    FacadeImportItemJs::AbiFunction(_) | FacadeImportItemJs::Runtime(_) => None,
                };

                (std::mem::discriminant(item), module, item.name(db))
            })
            .map(|item| FacadeImportJs::new(db, self, item))
            .collect()
    }
}

impl<'db> FacadeTrait<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItemJs<'db>> + 'db {
        self.specializations(db).iter().flat_map(|mapping| {
            mapping
                .arguments
                .iter()
                .filter_map(|argument| match argument {
                    FacadeTraitArgument::Value(FacadeGeneric::Class(class))
                    | FacadeTraitArgument::Reference { class, .. } => {
                        Some(FacadeImportItemJs::FacadeClass(*class))
                    }
                    FacadeTraitArgument::Value(FacadeGeneric::Symbol(_)) => None,
                })
        })
    }
}

impl<'db> FacadeClassTemplate<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItemJs<'db>> + 'db {
        self.mappings(db).iter().flat_map(move |(generics, class)| {
            generics
                .iter()
                .filter_map(|generic| match generic {
                    FacadeGeneric::Class(class) => Some(FacadeImportItemJs::FacadeClass(*class)),
                    FacadeGeneric::Symbol(_) => None,
                })
                .chain(class.imports(db))
        })
    }
}

#[salsa::tracked]
impl<'db> FacadeFreeFunction<'db> {
    #[salsa::tracked(returns(ref))]
    fn imports(self, db: &'db dyn Db) -> Vec<FacadeImportItemJs<'db>> {
        let mut imports = HashSet::from([FacadeImportItemJs::AbiFunction(self.abi(db))]);

        for type_ in self
            .params(db)
            .iter()
            .map(|param| param.type_js(db))
            .chain(std::iter::once(self.ret_type_js(db)))
            .chain(self.as_fn().ret_err_type_js(db))
        {
            if let FacadeTypeJs::Class(class) = type_ {
                imports.insert(FacadeImportItemJs::FacadeClass(class));
            }
        }

        imports.into_iter().collect()
    }
}

impl<'db> FacadeMethod<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItemJs<'db>> + 'db {
        self.params(db)
            .iter()
            .map(|param| param.type_js(db))
            .chain(std::iter::once(self.ret_type_js(db)))
            .chain(self.as_fn().ret_err_type_js(db))
            .filter_map(|type_| match type_ {
                FacadeTypeJs::Class(class) => Some(class),
                _ => None,
            })
            .map(FacadeImportItemJs::FacadeClass)
            .chain(std::iter::once(FacadeImportItemJs::AbiFunction(
                self.abi(db),
            )))
    }
}

impl<'db> FacadeCoerce<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItemJs<'db>> + 'db {
        match self {
            Self::Plain(coerce) => Either::Left(coerce.imports(db).into_iter()),
            Self::Discriminated(coerce) => Either::Left(coerce.imports(db).into_iter()),
            Self::Option(coerce) => {
                let imports = [
                    FacadeImportItemJs::AbiFunction(coerce.some(db).abi_coerce(db)),
                    FacadeImportItemJs::AbiFunction(coerce.none(db).abi_coerce(db)),
                    FacadeImportItemJs::FacadeClass(self.class(db)),
                ];
                let payload_imports =
                    coerce
                        .some(db)
                        .fields(db)
                        .filter_map(|field| match field.type_js(db) {
                            FacadeTypeJs::Class(class) => {
                                Some(FacadeImportItemJs::FacadeClass(class))
                            }
                            _ => None,
                        });

                Either::Right(imports.into_iter().chain(payload_imports))
            }
        }
    }
}

impl<'db> FacadePlainCoerceFunction<'db> {
    fn imports(self, db: &'db dyn Db) -> Vec<FacadeImportItemJs<'db>> {
        let class = FacadeClass::Plain(self.class(db));

        [
            FacadeImportItemJs::AbiFunction(self.abi(db)),
            FacadeImportItemJs::FacadeClass(class),
        ]
        .into_iter()
        .chain(self.fields(db).filter_map(|field| match field.type_js(db) {
            FacadeTypeJs::Class(class) => Some(FacadeImportItemJs::FacadeClass(class)),
            _ => None,
        }))
        .collect()
    }
}

#[salsa::tracked]
impl<'db> FacadeDiscriminatedCoerceFunction<'db> {
    fn imports(self, db: &'db dyn Db) -> Vec<FacadeImportItemJs<'db>> {
        let class = FacadeClass::Discriminated(self.class(db));

        self.coerce_variants(db)
            .iter()
            .flat_map(|variant| {
                [
                    FacadeImportItemJs::AbiFunction(variant.abi),
                    FacadeImportItemJs::FacadeClass(class),
                ]
                .into_iter()
                .chain(
                    variant
                        .fields
                        .iter()
                        .filter_map(|field| match field.type_js(db) {
                            FacadeTypeJs::Class(class) => {
                                Some(FacadeImportItemJs::FacadeClass(class))
                            }
                            _ => None,
                        }),
                )
            })
            .collect()
    }
}

#[salsa::tracked]
impl<'db> FacadeClass<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn import_alias(self, db: &'db dyn Db) -> String {
        match self {
            Self::Plain(class) => format!("$facade__plain__{}", class.src(db).path(db).mangled(db)),
            Self::Discriminated(class) => {
                format!("$facade__discr__{}", class.src(db).path(db).mangled(db))
            }
        }
    }

    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItemJs<'db>> + 'db {
        (match self {
            Self::Plain(class) => Either::Left(class.imports(db)),
            Self::Discriminated(class) => Either::Right(class.imports(db)),
        })
        .chain(
            self.coerce(db)
                .into_iter()
                .flat_map(move |coerce| coerce.imports(db)),
        )
        .chain(
            match self {
                Self::Plain(_) => None,
                Self::Discriminated(class) => Some(class.discriminator(db)),
            }
            .into_iter()
            .flat_map(move |discriminator| discriminator.imports(db)),
        )
        .chain(std::iter::once(FacadeImportItemJs::AbiFunction(
            self.drop_handle_fn(db).abi(db),
        )))
    }
}

impl<'db> FacadeClassPlain<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItemJs<'db>> + 'db {
        self.fields(db)
            .flat_map(|field| field.imports(db))
            .chain(self.methods(db).flat_map(|method| method.imports(db)))
    }
}

impl<'db> FacadeClassDiscriminated<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItemJs<'db>> + 'db {
        self.fields(db)
            .iter()
            .flat_map(|field| field.imports(db))
            .chain(self.methods(db).flat_map(|method| method.imports(db)))
    }
}

impl<'db> FacadeFieldPlain<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItemJs<'db>> {
        [
            Some(FacadeImportItemJs::AbiFunction(self.abi_getter(db))),
            self.abi_setter(db).map(FacadeImportItemJs::AbiFunction),
            match self.type_js(db) {
                FacadeTypeJs::Class(class) => Some(FacadeImportItemJs::FacadeClass(class)),
                _ => None,
            },
        ]
        .into_iter()
        .flatten()
    }
}

impl<'db> FacadeFieldDiscriminated<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItemJs<'db>> {
        self.sub(db).values().flat_map(|field| field.imports(db))
    }
}

impl<'db> FacadeFieldDiscriminator<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItemJs<'db>> {
        std::iter::once(FacadeImportItemJs::AbiFunction(self.abi_getter(db)))
    }
}
