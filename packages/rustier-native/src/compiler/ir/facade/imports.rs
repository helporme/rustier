use std::collections::HashSet;

use itertools::{Either, Itertools};

use crate::compiler::{
    db::Db,
    ir::{
        abi::{AbiClass, AbiFunction},
        facade::{
            FacadeClass, FacadeClassDiscriminated, FacadeClassPlain, FacadeClassTemplate,
            FacadeFieldDiscriminated, FacadeFieldDiscriminator, FacadeFieldPlain,
            FacadeFreeFunction, FacadeGeneric, FacadeMember, FacadeMethod, FacadeModule,
            coerce::{FacadeDiscriminatedCoerceFunction, FacadePlainCoerceFunction},
            js_type::FacadeJsType,
        },
    },
};

#[salsa::tracked(debug)]
pub struct FacadeImport<'db> {
    pub originating_from: FacadeModule<'db>,
    pub item: FacadeImportItem<'db>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeImportItem<'db> {
    AbiFunction(AbiFunction<'db>),
    AbiClass(AbiClass<'db>),
    FacadeClass(FacadeClass<'db>),
}

impl<'db> FacadeImportItem<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self {
            Self::AbiFunction(f) => f.symbol(db),
            Self::AbiClass(c) => c.symbol(db),
            Self::FacadeClass(c) => c.name(db),
        }
    }
}

#[salsa::tracked]
impl<'db> FacadeModule<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn imports(self, db: &'db dyn Db) -> Vec<FacadeImport<'db>> {
        let mut items = Vec::new();

        for member in self.members(db).iter().copied() {
            match member {
                FacadeMember::Function(function) => {
                    items.extend(function.imports(db).iter().copied())
                }
                FacadeMember::Class(class) => items.extend(class.imports(db)),
                FacadeMember::ClassTemplate(template) => items.extend(template.imports(db)),
                FacadeMember::Module(_) => {}
            }
        }

        items.into_iter()
            .filter(|item| !matches!(item, FacadeImportItem::FacadeClass(class) if class.module(db) == self.as_parent(db)))
            .unique_by(|item| {
                let module = match item {
                    FacadeImportItem::FacadeClass(class) => Some(class.module(db)),
                    FacadeImportItem::AbiClass(_) | FacadeImportItem::AbiFunction(_) => None,
                };

                (module, item.name(db))
            })
            .map(|item| FacadeImport::new(db, self, item))
            .collect()
    }
}

impl<'db> FacadeClassTemplate<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> + 'db {
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
}

#[salsa::tracked]
impl<'db> FacadeFreeFunction<'db> {
    #[salsa::tracked(returns(ref))]
    fn imports(self, db: &'db dyn Db) -> Vec<FacadeImportItem<'db>> {
        let mut imports = HashSet::from([FacadeImportItem::AbiFunction(self.abi(db))]);

        if let Some(FacadeJsType::Class(class)) = self.ret_js_type(db) {
            imports.insert(FacadeImportItem::AbiClass(class.abi(db)));
        }

        for type_ in self
            .params(db)
            .map(|param| param.js_type(db))
            .chain(self.ret_js_type(db))
        {
            if let FacadeJsType::Class(class) = type_ {
                imports.insert(FacadeImportItem::FacadeClass(class));
            }
        }

        imports.into_iter().collect()
    }
}

impl<'db> FacadeMethod<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> + 'db {
        self.params(db)
            .map(|param| param.js_type(db))
            .chain(self.ret_js_type(db))
            .filter_map(|type_| match type_ {
                FacadeJsType::Class(class) => Some(class),
                _ => None,
            })
            .map(FacadeImportItem::FacadeClass)
            .chain(std::iter::once(FacadeImportItem::AbiFunction(self.abi(db))))
            .chain(self.ret_js_type(db).and_then(|type_| match type_ {
                FacadeJsType::Class(class) => Some(FacadeImportItem::AbiClass(class.abi(db))),
                _ => None,
            }))
    }
}

impl<'db> FacadePlainCoerceFunction<'db> {
    fn imports(self, db: &'db dyn Db) -> Vec<FacadeImportItem<'db>> {
        let class = FacadeClass::Plain(self.class(db));

        [
            FacadeImportItem::AbiFunction(self.abi(db)),
            FacadeImportItem::AbiClass(class.abi(db)),
            FacadeImportItem::FacadeClass(class),
        ]
        .into_iter()
        .chain(self.fields(db).filter_map(|field| match field.js_type(db) {
            FacadeJsType::Class(class) => Some(FacadeImportItem::FacadeClass(class)),
            _ => None,
        }))
        .collect()
    }
}

#[salsa::tracked]
impl<'db> FacadeDiscriminatedCoerceFunction<'db> {
    fn imports(self, db: &'db dyn Db) -> Vec<FacadeImportItem<'db>> {
        let class = FacadeClass::Discriminated(self.class(db));

        self.coerce_variants(db)
            .iter()
            .flat_map(|variant| {
                [
                    FacadeImportItem::AbiFunction(variant.abi),
                    FacadeImportItem::AbiClass(class.abi(db)),
                    FacadeImportItem::FacadeClass(class),
                ]
                .into_iter()
                .chain(
                    variant
                        .fields
                        .iter()
                        .filter_map(|field| match field.js_type(db) {
                            FacadeJsType::Class(class) => {
                                Some(FacadeImportItem::FacadeClass(class))
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
        format!("$facade{}", self.abi(db).symbol(db))
    }

    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> + 'db {
        match self {
            Self::Plain(class) => Either::Left(class.imports(db)),
            Self::Discriminated(class) => Either::Right(class.imports(db)),
        }
    }
}

impl<'db> FacadeClassPlain<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> + 'db {
        self.fields(db)
            .flat_map(|field| field.imports(db))
            .chain(self.methods(db).flat_map(|method| method.imports(db)))
            .chain(
                self.coerce(db)
                    .into_iter()
                    .flat_map(move |coerce| coerce.imports(db)),
            )
            .chain(std::iter::once(FacadeImportItem::AbiClass(self.abi(db))))
    }
}

impl<'db> FacadeClassDiscriminated<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> + 'db {
        self.fields(db)
            .iter()
            .flat_map(|field| field.imports(db))
            .chain(self.methods(db).flat_map(|method| method.imports(db)))
            .chain(
                self.coerce(db)
                    .into_iter()
                    .flat_map(move |coerce| coerce.imports(db)),
            )
            .chain(self.discriminator(db).imports(db))
    }
}

impl<'db> FacadeFieldPlain<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> {
        [
            Some(FacadeImportItem::AbiFunction(self.abi_getter(db))),
            self.abi_setter(db).map(FacadeImportItem::AbiFunction),
            match self.js_type(db) {
                FacadeJsType::Class(class) => Some(FacadeImportItem::FacadeClass(class)),
                _ => None,
            },
            match self.js_type(db) {
                FacadeJsType::Class(class) => Some(FacadeImportItem::AbiClass(class.abi(db))),
                _ => None,
            },
        ]
        .into_iter()
        .flatten()
    }
}

impl<'db> FacadeFieldDiscriminated<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> {
        self.sub(db).values().flat_map(|field| field.imports(db))
    }
}

impl<'db> FacadeFieldDiscriminator<'db> {
    fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> {
        std::iter::once(FacadeImportItem::AbiFunction(self.abi_getter(db)))
    }
}
