use crate::compiler::{
    db::Db,
    ir::{
        ffi::{FfiClass, FfiFunction},
        interface::{
            InterClass, InterClassDiscriminated, InterClassDiscriminatedVariant, InterClassPlain,
            InterField, InterFunctionAssociated, InterFunctionFree,
        },
        runtime::{RuntimeDiscriminatedCoerce, RuntimeFunction, RuntimePlainCoerce},
    },
};

impl<'db> InterClass<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        FfiClass::new(db, self)
    }
}

impl<'db> InterClassPlain<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        FfiClass::new(db, InterClass::Plain(self))
    }

    pub fn ffi_coerce(self, db: &'db dyn Db) -> FfiFunction<'db> {
        RuntimeFunction::from(RuntimePlainCoerce::new(db, self)).ffi(db)
    }
}

impl<'db> InterClassDiscriminated<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        FfiClass::new(db, InterClass::Discriminated(self))
    }

    pub fn ffi_discriminator(self, db: &'db dyn Db) -> FfiFunction<'db> {
        RuntimeFunction::from(self.discriminator(db)).ffi(db)
    }
}

impl<'db> InterClassDiscriminatedVariant<'db> {
    pub fn ffi_coerce(self, db: &'db dyn Db) -> FfiFunction<'db> {
        RuntimeFunction::from(RuntimeDiscriminatedCoerce::new(db, self)).ffi(db)
    }
}

impl<'db> InterField<'db> {
    pub fn ffi_getter(self, db: &'db dyn Db) -> FfiFunction<'db> {
        RuntimeFunction::from(self.getter(db)).ffi(db)
    }

    pub fn ffi_setter(self, db: &'db dyn Db) -> Option<FfiFunction<'db>> {
        self.setter(db)
            .map(|setter| RuntimeFunction::from(setter).ffi(db))
    }
}

impl<'db> InterFunctionFree<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        RuntimeFunction::from(self).ffi(db)
    }
}

impl<'db> InterFunctionAssociated<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        RuntimeFunction::from(self).ffi(db)
    }
}

impl<'db> RuntimeFunction<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, self)
    }
}
