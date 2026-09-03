//! Intermediate representation used to generate the JavaScript API.

pub mod ext;
mod lowering;

use std::collections::{BTreeMap, HashMap, HashSet};

use itertools::Itertools;

use crate::compiler::{
    db::Db,
    inputs::FilePaths,
    ir::{
        ffi::{FfiClass, FfiFunction},
        rust::{FieldName, Function},
        shim::{
            ShimClassDiscriminated, ShimClassPlain, ShimField, ShimFunctionAssociated,
            ShimFunctionDiscriminator, ShimFunctionFree, ShimMember, ShimModule, ShimParam,
            ShimType,
        },
    },
    mangle::to_camel_case,
    path::FilePath,
};

/// An import of a facade item.
///
/// Matching the js `import`/ statement.
#[salsa::tracked(debug)]
pub struct FacadeImport<'db> {
    /// The module where the import statement will be emitted.
    pub originating_from: FacadeModule<'db>,
    pub item: FacadeImportItem<'db>,
}

#[salsa::tracked]
impl<'db> FacadeImport<'db> {
    /// The path to import an item from.
    #[salsa::tracked(returns(ref))]
    pub fn from_path(self, db: &'db dyn Db) -> String {
        let layout = FilePaths::get(db);
        let originating_from_path = self.originating_from(db).file_path(db);
        let to_path = match self.item(db) {
            FacadeImportItem::FfiFunction(_) | FacadeImportItem::FfiClass(_) => &layout.ffi_js_file,
            FacadeImportItem::FacadeClass(class) => class.module(db).file_path(db),
        };

        originating_from_path
            .relative_path(to_path)
            .expect("relative path must exist")
    }
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
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        match self {
            FacadeImportItem::FfiFunction(f) => f.symbol(db),
            FacadeImportItem::FfiClass(c) => c.symbol(db),
            FacadeImportItem::FacadeClass(c) => c.ffi(db).symbol(db),
        }
    }
}

/// A facade module.
///
/// Matches a single `.js` file generated for a rust module. Take into accout
/// that there is no crate containing this module since crates aren't a thing in
/// js.
#[salsa::interned(debug)]
pub struct FacadeModule<'db> {
    shim: ShimModule<'db>,
}

#[salsa::tracked]
impl<'db> FacadeModule<'db> {
    /// Members defined in this module.
    pub fn members(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeMember<'db>> + 'db {
        self.shim(db).members(db).iter().map(|member| match member {
            ShimMember::Class(class) => FacadeMember::Class(class.facade(db)),
            ShimMember::Function(function) => FacadeMember::Function(function.facade(db)),
            ShimMember::Module(module) => FacadeMember::Module(module.facade(db)),
        })
    }

    /// Imports needed for this module.
    #[salsa::tracked(returns(ref))]
    pub fn imports(self, db: &'db dyn Db) -> Vec<FacadeImport<'db>> {
        let mut items = HashSet::new();

        for member in self.members(db) {
            match member {
                FacadeMember::Function(function) => {
                    items.extend(function.imports(db).iter().copied());
                }
                FacadeMember::Class(class) => {
                    items.extend(class.imports(db).iter().copied());
                }
                FacadeMember::Module(_) => {}
            }
        }

        items
            .into_iter()
            .filter(|item| {
                !matches!(
                    item,
                    FacadeImportItem::FacadeClass(class) if class.module(db) == self
                )
            })
            .map(|item| FacadeImport::new(db, self, item))
            .collect()
    }

    /// File path of the source rust module.
    pub fn file_path(self, db: &'db dyn Db) -> &'db FilePath {
        self.shim(db).src(db).file_path(db)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeMember<'db> {
    Class(FacadeClass<'db>),
    Function(FacadeFreeFunction<'db>),
    Module(FacadeModule<'db>),
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

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<FacadeParam<'db>> {
        match self {
            Self::Free(function) => function.params(db).collect(),
            Self::Method(method) => method.params(db).collect(),
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
        self.src(db).name(db)
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

    fn src(self, db: &'db dyn Db) -> Function<'db> {
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
        self.src(db).receiver(db).is_none()
    }

    pub fn class(self, db: &'db dyn Db) -> FacadeClass<'db> {
        self.shim(db).self_class(db).facade(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).name(db)
    }

    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).ffi(db)
    }

    pub fn params(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeParam<'db>> + 'db {
        self.shim(db)
            .params_without_self(db)
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
            .flat_map(|cls| cls.imports(db).iter().copied())
            .chain(std::iter::once(FacadeImportItem::FfiFunction(self.ffi(db))))
    }

    fn src(self, db: &'db dyn Db) -> Function<'db> {
        self.shim(db).src(db)
    }
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

    pub fn module(self, db: &'db dyn Db) -> FacadeModule<'db> {
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

    // pub fn kind(self, db: &'db dyn Db) -> &'db FacadeClassKind<'db> {
    //     self.lower_kind(db)
    // }

    #[salsa::tracked(returns(ref))]
    pub fn methods(self, db: &'db dyn Db) -> Vec<FacadeMethod<'db>> {
        match self {
            FacadeClass::Plain(plain) => plain.methods(db).collect(),
            FacadeClass::Discriminated(discriminated) => discriminated.methods(db).collect(),
        }
    }

    pub fn constructor(self, db: &'db dyn Db) -> Option<FacadeMethod<'db>> {
        match self {
            FacadeClass::Plain(plain) => plain.constructor(db),
            FacadeClass::Discriminated(_) => None,
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn imports(self, db: &'db dyn Db) -> Vec<FacadeImportItem<'db>> {
        match self {
            FacadeClass::Plain(plain) => plain.imports(db).collect(),
            FacadeClass::Discriminated(discriminated) => discriminated.imports(db).collect(),
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

    pub fn module(self, db: &'db dyn Db) -> FacadeModule<'db> {
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
                    }) if inner == self.shim(db).into()
                )
        })
    }

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> + 'db {
        self.fields(db)
            .flat_map(|f| f.imports(db))
            .chain(self.methods(db).flat_map(|m| m.imports(db)))
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

    pub fn module(self, db: &'db dyn Db) -> FacadeModule<'db> {
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

    pub fn imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeImportItem<'db>> + 'db {
        self.fields(db)
            .iter()
            .flat_map(|f| f.imports(db))
            .chain(self.methods(db).flat_map(|m| m.imports(db)))
            .chain(self.discriminator(db).imports(db))
    }
}

#[salsa::interned(debug)]
pub struct FacadeFieldPlain<'db> {
    shim: ShimField<'db>,
}

impl<'db> FacadeFieldPlain<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db FieldName {
        self.shim(db).src(db).name(db)
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

        if enum_.variants(db).iter().any(|variant| {
            variant
                .fields(db)
                .iter()
                .any(|field| matches!(field.name(db), FieldName::Name(name) if name == "type"))
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
