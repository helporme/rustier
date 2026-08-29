//! The main intermediate representation used by rust-curse.
//!
//! Shim IR is the bridge between the supported Rust source API and the
//! generated JavaScript-facing API. It is used directly to generate the Rust
//! shim code and is also the source representation lowered into the facade and
//! FFI IRs.
//!
//! Unlike the downstream IRs, shim entities *deliberately* keep references to
//! the higher-level Rust IR public.

pub mod ext;
mod lowering;

use crate::compiler::{
    db::Db,
    ir::rust::{Crate, Enum, Field, Function, IntoValueType, Mod, Struct, Type, ValueType},
};

#[salsa::interned(debug)]
pub struct Shim<'db> {
    #[returns(ref)]
    pub crates: Vec<ShimCrate<'db>>,
}

#[salsa::interned(debug)]
pub struct ShimCrate<'db> {
    pub src: Crate<'db>,
}

impl<'db> ShimCrate<'db> {
    pub fn mods(self, db: &'db dyn Db) -> &'db Vec<ShimMod<'db>> {
        self.lower_mods(db)
    }
}

#[salsa::interned(debug)]
pub struct ShimMod<'db> {
    pub src: Mod<'db>,
}

impl<'db> ShimMod<'db> {
    pub fn classes(self, db: &'db dyn Db) -> &'db Vec<ShimClass<'db>> {
        self.lower_classes(db)
    }

    pub fn fns(self, db: &'db dyn Db) -> &'db Vec<ShimFunction<'db>> {
        self.lower_fns(db)
    }
}

/// A JavaScript-facing class lowered from a Rust struct or enum.
#[salsa::interned(debug)]
pub struct ShimClass<'db> {
    pub src: ShimClassSource<'db>,
}

impl<'db> ShimClass<'db> {
    pub fn parent(self, db: &'db dyn Db) -> ShimMod<'db> {
        self.lower_parent(db)
    }

    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.lower_symbol(db)
    }

    pub fn fields(self, db: &'db dyn Db) -> &'db Vec<ShimField<'db>> {
        self.lower_fields(db)
    }

    pub fn associated_fns(self, db: &'db dyn Db) -> &'db Vec<ShimFunctionAssociated<'db>> {
        self.lower_associated_fns(db)
    }

    pub(super) fn fns(self, db: &'db dyn Db) -> &'db Vec<ShimFunction<'db>> {
        self.lower_fns(db)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ShimClassSource<'db> {
    Struct(Struct<'db>),
    Enum(Enum<'db>),
}

impl<'db> IntoValueType<'db> for ShimClassSource<'db> {
    fn into_value_type(self) -> ValueType<'db> {
        match self {
            ShimClassSource::Struct(s) => s.into_value_type(),
            ShimClassSource::Enum(e) => e.into_value_type(),
        }
    }
}

#[salsa::interned(debug)]
pub struct ShimField<'db> {
    pub src: Field<'db>,
}

impl<'db> ShimField<'db> {
    pub fn getter(self, db: &'db dyn Db) -> ShimFunctionGetter<'db> {
        ShimFunctionGetter::new(db, self.src(db))
    }

    pub fn setter(self, db: &'db dyn Db) -> Option<ShimFunctionSetter<'db>> {
        let src = self.src(db);
        (!src.is_readonly(db)).then(|| ShimFunctionSetter::new(db, src))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ShimFunction<'db> {
    /// A function declared in the source API.
    Free(ShimFunctionFree<'db>),
    /// A source-defined function in an inherent impl block.
    Associated(ShimFunctionAssociated<'db>),
    /// A synthetic field-read operation.
    Getter(ShimFunctionGetter<'db>),
    /// A synthetic field-write operation.
    Setter(ShimFunctionSetter<'db>),
    /// A synthetic operation that identifies an enum variant.
    EnumDiscriminator(ShimFunctionEnumDiscriminator<'db>),
}

impl<'db> ShimFunction<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        match self {
            ShimFunction::Free(f) => f.symbol(db),
            ShimFunction::Associated(f) => f.symbol(db),
            ShimFunction::Getter(g) => g.symbol(db),
            ShimFunction::Setter(s) => s.symbol(db),
            ShimFunction::EnumDiscriminator(e) => e.symbol(db),
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimType<'db>> {
        match self {
            ShimFunction::Free(f) => f.ret_type(db),
            ShimFunction::Associated(f) => f.ret_type(db),
            ShimFunction::Getter(g) => Some(g.ret_type(db)),
            ShimFunction::Setter(_) => None,
            ShimFunction::EnumDiscriminator(d) => Some(d.ret_type()),
        }
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<ShimParam<'db>> {
        self.lower_params(db)
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionFree<'db> {
    pub src: Function<'db>,
}

impl<'db> ShimFunctionFree<'db> {
    pub fn as_fn(self) -> ShimFunction<'db> {
        ShimFunction::Free(self)
    }

    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.lower_symbol(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimType<'db>> {
        self.lower_ret_type(db)
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<ShimParam<'db>> {
        self.lower_params(db)
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionAssociated<'db> {
    pub src: Function<'db>,
}

impl<'db> ShimFunctionAssociated<'db> {
    pub fn as_fn(self) -> ShimFunction<'db> {
        ShimFunction::Associated(self)
    }

    pub fn self_class(self, db: &'db dyn Db) -> ShimClass<'db> {
        self.lower_self_class(db)
    }

    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.lower_symbol(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimType<'db>> {
        self.lower_ret_type(db)
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<ShimParam<'db>> {
        self.lower_params(db)
    }

    pub fn params_without_receiver(self, db: &'db dyn Db) -> &'db [ShimParam<'db>] {
        let offset = usize::from(self.src(db).receiver(db).is_some());
        &self.params(db)[offset..]
    }
}

#[salsa::tracked(debug)]
pub struct ShimParam<'db> {
    pub parent: ShimFunction<'db>,
    pub idx: usize,
    #[tracked]
    pub src_type: Type<'db>,
    #[tracked]
    pub type_: ShimType<'db>,
}

#[salsa::interned(debug)]
pub struct ShimFunctionGetter<'db> {
    pub src: Field<'db>,
}

impl<'db> ShimFunctionGetter<'db> {
    pub fn as_fn(self) -> ShimFunction<'db> {
        ShimFunction::Getter(self)
    }

    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.lower_symbol(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> ShimType<'db> {
        self.lower_ret_type(db)
    }

    pub fn params(self, db: &'db dyn Db) -> &'db [ShimParam<'db>; 1] {
        self.lower_params(db)
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionSetter<'db> {
    pub src: Field<'db>,
}

impl<'db> ShimFunctionSetter<'db> {
    pub fn as_fn(self) -> ShimFunction<'db> {
        ShimFunction::Setter(self)
    }

    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.lower_symbol(db)
    }

    pub fn params(self, db: &'db dyn Db) -> &'db [ShimParam<'db>; 2] {
        self.lower_params(db)
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionEnumDiscriminator<'db> {
    pub src: Enum<'db>,
}

impl<'db> ShimFunctionEnumDiscriminator<'db> {
    pub fn as_fn(self) -> ShimFunction<'db> {
        ShimFunction::EnumDiscriminator(self)
    }

    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        self.lower_symbol(db)
    }

    pub fn ret_type(self) -> ShimType<'db> {
        ShimType::f64
    }

    pub fn class(self, db: &'db dyn Db) -> ShimClass<'db> {
        self.src(db).shim(db)
    }

    pub fn params(self, db: &'db dyn Db) -> &'db [ShimParam<'db>; 1] {
        self.lower_params(db)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum ShimType<'db> {
    f64,
    String,
    bool,
    Class {
        is_borrowed: bool,
        inner: ShimClass<'db>,
    },
}
