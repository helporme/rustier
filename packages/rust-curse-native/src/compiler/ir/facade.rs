use crate::compiler::{
    db::Db,
    ir::{
        ffi::{FfiClass, FfiFunction},
        shim::{ShimClass, ShimCrate, ShimField, ShimFunction, ShimMod, ShimParam},
    },
};

#[salsa::interned(debug)]
pub struct FacadeCrate<'db> {
    shim: ShimCrate<'db>,
}

#[salsa::interned(debug)]
pub struct FacadeMod<'db> {
    shim: ShimMod<'db>,
}

#[salsa::interned(debug)]
pub struct FacadeFunction<'db> {
    shim: ShimFunction<'db>,
}

#[salsa::interned(debug)]
pub struct FacadeParam<'db> {
    shim: ShimParam<'db>,
}

#[salsa::interned(debug)]
pub struct FacadeClass<'db> {
    shim: ShimClass<'db>,
}

impl<'db> FacadeClass<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass {
        self.shim(db).ffi(db)
    }

    pub fn kind(self, db: &'db dyn Db) -> &FacadeClassKind<'db> {
        todo!()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, salsa::Update)]
pub enum FacadeClassKind<'db> {
    Plain(Vec<FacadeFieldPlain<'db>>),
    TagDiscriminated {
        fields: Vec<FacadeFieldDiscriminated<'db>>,
        discriminator: FacadeFieldDiscriminator<'db>,
        variant_names: Vec<String>,
    },
}

#[salsa::interned(debug)]
pub struct FacadeFieldPlain<'db> {
    shim: ShimField<'db>,
}

impl<'db> FacadeFieldPlain<'db> {
    pub fn ffi_setter(self, db: &'db dyn Db) -> Option<FfiFunction<'db>> {
        self.shim(db).setter(db).map(|f| f.ffi(db))
    }

    pub fn ffi_getter(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).getter(db).ffi(db)
    }
}

#[salsa::interned(debug)]
pub struct FacadeFieldDiscriminated<'db> {
    #[returns(ref)]
    pub variants: Vec<FacadeFieldPlain<'db>>,
    discriminator: u32,
}

#[salsa::interned(debug)]
pub struct FacadeFieldDiscriminator<'db> {
    shim: ShimFunction<'db>,
}
