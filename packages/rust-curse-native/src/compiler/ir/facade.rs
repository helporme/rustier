//! Intermediate representation used to generate the JavaScript API.

pub mod ext;
mod lowering;

use std::collections::{BTreeMap, HashSet};

use crate::compiler::{
    db::Db,
    ir::{
        ffi::{FfiClass, FfiFunction},
        rust::{FieldName, Function},
        shim::{
            ShimClass, ShimClassSource, ShimField, ShimFunctionAssociated,
            ShimFunctionEnumDiscriminator, ShimFunctionFree, ShimMod, ShimParam,
        },
    },
    path::FilePath,
};

#[salsa::tracked(debug)]
pub struct FacadeImport<'db> {
    /// The module where the import statement will be emitted.
    pub originating_from: FacadeMod<'db>,
    pub item: FacadeImportItem<'db>,
}

impl<'db> FacadeImport<'db> {
    pub fn path(self, db: &'db dyn Db) -> &'db str {
        self.lower_path(db)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeImportItem<'db> {
    FfiFunction(FfiFunction<'db>),
    FfiClass(FfiClass<'db>),
    FacadeClass(FacadeClass<'db>),
}

impl<'db> FacadeImportItem<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        match self {
            FacadeImportItem::FfiFunction(f) => f.symbol(db),
            FacadeImportItem::FfiClass(c) => c.symbol(db),
            FacadeImportItem::FacadeClass(c) => c.ffi(db).symbol(db),
        }
    }
}

#[salsa::interned(debug)]
pub struct FacadeMod<'db> {
    shim: ShimMod<'db>,
}

impl<'db> FacadeMod<'db> {
    pub fn fns(self, db: &'db dyn Db) -> &'db Vec<FacadeFreeFunction<'db>> {
        self.lower_fns(db)
    }

    pub fn classes(self, db: &'db dyn Db) -> &'db Vec<FacadeClass<'db>> {
        self.lower_classes(db)
    }

    pub fn imports(self, db: &'db dyn Db) -> &'db HashSet<FacadeImport<'db>> {
        self.lower_imports(db)
    }

    pub fn file_path(self, db: &'db dyn Db) -> &'db FilePath {
        self.lower_file_path(db)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeFunction<'db> {
    Free(FacadeFreeFunction<'db>),
    Method(FacadeMethod<'db>),
}

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

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<FacadeParam<'db>> {
        match self {
            Self::Free(function) => function.params(db),
            Self::Method(method) => method.params(db),
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

impl<'db> FacadeFreeFunction<'db> {
    pub fn as_fn(self) -> FacadeFunction<'db> {
        FacadeFunction::Free(self)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).name(db)
    }

    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).as_fn().ffi(db)
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<FacadeParam<'db>> {
        self.lower_params(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<FacadeType<'db>> {
        self.lower_ret_type(db)
    }

    pub(super) fn imports(self, db: &'db dyn Db) -> &'db HashSet<FacadeImportItem<'db>> {
        self.lower_imports(db)
    }

    fn src(self, db: &'db dyn Db) -> Function<'db> {
        self.shim(db).src(db)
    }
}

#[salsa::interned(debug)]
pub struct FacadeMethod<'db> {
    shim: ShimFunctionAssociated<'db>,
}

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
        self.shim(db).as_fn().ffi(db)
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<FacadeParam<'db>> {
        self.lower_params(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<FacadeType<'db>> {
        self.lower_ret_type(db)
    }

    pub(super) fn imports(self, db: &'db dyn Db) -> &'db HashSet<FacadeImportItem<'db>> {
        self.lower_imports(db)
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
        self.lower_type(db)
    }
}

#[salsa::interned(debug)]
pub struct FacadeClass<'db> {
    shim: ShimClass<'db>,
}

impl<'db> FacadeClass<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        self.shim(db).ffi(db)
    }

    pub fn parent(self, db: &'db dyn Db) -> FacadeMod<'db> {
        self.shim(db).parent(db).facade(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self.shim(db).src(db) {
            ShimClassSource::Struct(s) => s.name(db),
            ShimClassSource::Enum(e) => e.name(db),
        }
    }

    pub fn kind(self, db: &'db dyn Db) -> &'db FacadeClassKind<'db> {
        self.lower_kind(db)
    }

    pub fn methods(self, db: &'db dyn Db) -> &'db Vec<FacadeMethod<'db>> {
        self.lower_methods(db)
    }

    pub fn constructor(self, db: &'db dyn Db) -> Option<FacadeMethod<'db>> {
        self.lower_constructor(db)
    }

    pub(super) fn imports(self, db: &'db dyn Db) -> &'db HashSet<FacadeImportItem<'db>> {
        self.lower_imports(db)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, salsa::Update)]
pub enum FacadeClassKind<'db> {
    Plain {
        fields: Vec<FacadeFieldPlain<'db>>,
    },
    DiscriminatedUnion {
        fields: Vec<FacadeFieldDiscriminated<'db>>,
        discriminator: FacadeFieldDiscriminator<'db>,
    },
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
        self.lower_type(db)
    }
}

#[salsa::tracked(debug)]
pub struct FacadeFieldDiscriminated<'db> {
    parent: FacadeClass<'db>,
    #[returns(ref)]
    pub name: FieldName,
    #[returns(ref)]
    #[tracked]
    pub sub: BTreeMap<u32, FacadeFieldPlain<'db>>,
}

#[salsa::interned(debug)]
pub struct FacadeFieldDiscriminator<'db> {
    shim: ShimFunctionEnumDiscriminator<'db>,
}

impl<'db> FacadeFieldDiscriminator<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.lower_name(db)
    }

    pub fn variant_names(self, db: &'db dyn Db) -> &'db Vec<String> {
        self.lower_variant_names(db)
    }

    pub fn ffi_getter(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).ffi(db)
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
