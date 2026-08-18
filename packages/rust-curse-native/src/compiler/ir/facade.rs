//! Intermediate representation used to generate the ergonomic JavaScript API
//! presented to users ("facade").
//!
//! Facade entities reference the FFI functions and classes that perform the
//! actual calls across the Rust-JavaScript boundary. In other words, this IR
//! describes how JavaScript code should be rendered over the lower-level API
//! represented by the FFI IR.

pub mod ext;
mod lowering;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::compiler::{
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

    pub fn file_path(self, db: &'db dyn Db) -> &'db PathBuf {
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

    pub fn fields(self, db: &'db dyn Db) -> &'db Vec<FacadeField<'db>> {
        self.lower_fields(db)
    }

    pub(super) fn imports(self, db: &'db dyn Db) -> &'db HashSet<FacadeImportItem<'db>> {
        self.lower_imports(db)
    }
}

/// A JavaScript-facing field, including enum-specific and synthetic fields.
#[derive(Debug, Copy, Clone, Eq, PartialEq, salsa::Update, salsa::Supertype)]
pub enum FacadeField<'db> {
    /// Has one representation independent of any enum variant.
    Plain(FacadeFieldPlain<'db>),
    /// Selects a variant-specific representation using an enum discriminator.
    Discriminated(FacadeFieldDiscriminated<'db>),
    /// Exposes an enum's active variant as a synthetic field.
    Discriminator(FacadeFieldDiscriminator<'db>),
}

/// A field whose representation does not vary by enum variant.
#[salsa::interned(debug)]
pub struct FacadeFieldPlain<'db> {
    shim: ShimField<'db>,
}

impl<'db> FacadeFieldPlain<'db> {
    pub fn as_field(self) -> FacadeField<'db> {
        FacadeField::Plain(self)
    }

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
    pub sub: HashMap<u32, FacadeFieldPlain<'db>>,
}

impl<'db> FacadeFieldDiscriminated<'db> {
    pub fn as_field(self) -> FacadeField<'db> {
        FacadeField::Discriminated(self)
    }
}

/// A synthetic field exposing which variant of an enum is active.
#[salsa::tracked(debug)]
pub struct FacadeFieldDiscriminator<'db> {
    shim: ShimFunctionEnumDiscriminator<'db>,
    /// Maps each numeric discriminator to the corresponding variant name by
    /// index.
    #[returns(ref)]
    #[tracked]
    pub variant_names: Vec<String>,
}

impl<'db> FacadeFieldDiscriminator<'db> {
    pub fn as_field(self) -> FacadeField<'db> {
        FacadeField::Discriminator(self)
    }

    /// The discriminator is emitted as a `type` property in JavaScript.
    pub fn name(self) -> &'db str {
        "type"
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
