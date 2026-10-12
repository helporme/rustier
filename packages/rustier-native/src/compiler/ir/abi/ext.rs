use crate::compiler::{
    db::Db,
    ir::{
        abi::{AbiClass, AbiFunction},
        interface::{
            InterClass, InterClassDiscriminated, InterClassDiscriminatedVariant, InterClassPlain,
            InterField, InterFunctionAssociated, InterFunctionFree,
        },
        runtime::{RuntimeDiscriminatedCoerce, RuntimeFunction, RuntimePlainCoerce},
    },
};

impl<'db> InterClass<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiClass<'db> {
        AbiClass::new(db, self)
    }
}

impl<'db> InterClassPlain<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiClass<'db> {
        AbiClass::new(db, InterClass::Plain(self))
    }

    pub fn abi_coerce(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(RuntimePlainCoerce::new(db, self)).abi(db)
    }
}

impl<'db> InterClassDiscriminated<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiClass<'db> {
        AbiClass::new(db, InterClass::Discriminated(self))
    }

    pub fn abi_discriminator(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(self.discriminator(db)).abi(db)
    }
}

impl<'db> InterClassDiscriminatedVariant<'db> {
    pub fn abi_coerce(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(RuntimeDiscriminatedCoerce::new(db, self)).abi(db)
    }
}

impl<'db> InterField<'db> {
    pub fn abi_getter(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(self.getter(db)).abi(db)
    }

    pub fn abi_setter(self, db: &'db dyn Db) -> Option<AbiFunction<'db>> {
        self.setter(db)
            .map(|setter| RuntimeFunction::from(setter).abi(db))
    }
}

impl<'db> InterFunctionFree<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(self).abi(db)
    }
}

impl<'db> InterFunctionAssociated<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(self).abi(db)
    }
}

impl<'db> RuntimeFunction<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiFunction<'db> {
        AbiFunction::new(db, self)
    }
}
