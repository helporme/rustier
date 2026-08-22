//! Intermediate representation used to generate the JavaScript API.

pub mod ext;
mod lowering;

use std::collections::{BTreeMap, HashSet};

use crate::compiler::{
    FilePath,
    db::Db,
    ir::{
        ffi::{FfiClass, FfiFunction},
        rust::FieldName,
        shim::{
            ShimClass, ShimClassSource, ShimField, ShimFunctionEnumDiscriminator, ShimFunctionFree,
            ShimMod, ShimParam,
        },
    },
};

/// Carries the importing module so its generated import path can be relative.
#[salsa::tracked(debug)]
pub struct FacadeImport<'db> {
    /// The module where the import statement will be emitted.
    pub originating_from: FacadeMod<'db>,
    pub item: FacadeImportItem<'db>,
}

impl<'db> FacadeImport<'db> {
    /// Resolves the imported item relative to the module emitting the import.
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
    pub fn fns(self, db: &'db dyn Db) -> &'db Vec<FacadeFunction<'db>> {
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

#[salsa::interned(debug)]
pub struct FacadeFunction<'db> {
    shim: ShimFunctionFree<'db>,
}

impl<'db> FacadeFunction<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.shim(db).src(db).name(db)
    }

    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).ffi(db)
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

    pub(super) fn imports(self, db: &'db dyn Db) -> &'db HashSet<FacadeImportItem<'db>> {
        self.lower_imports(db)
    }
}

/// The structural shape of a JavaScript facade class.
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

/// A field whose representation does not vary by enum variant.
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

/// Groups same-named enum fields whose representation differs between variants.
#[salsa::tracked(debug)]
pub struct FacadeFieldDiscriminated<'db> {
    parent: FacadeClass<'db>,
    /// The JavaScript-facing name shared by the variant-specific fields.
    #[returns(ref)]
    pub name: FieldName,
    /// Maps discriminator values to their variant-specific field accessors.
    #[returns(ref)]
    #[tracked]
    pub sub: BTreeMap<u32, FacadeFieldPlain<'db>>,
}

/// A synthetic field exposing which variant of an enum is active.
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
