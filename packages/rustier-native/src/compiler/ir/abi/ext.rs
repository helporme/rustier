use crate::compiler::{
    db::Db,
    ir::{
        abi::{Abi, AbiClass, AbiFunction},
        facade::{
            Facade, FacadeClass, FacadeClassDiscriminated, FacadeClassPlain,
            FacadeFieldDiscriminator, FacadeFieldPlain, FacadeFreeFunction, FacadeFunction,
            FacadeMethod, FacadeVariant, coerce::FacadePlainCoerceFunction,
        },
        runtime::{RuntimeDiscriminatedCoerce, RuntimeFunction, RuntimePlainCoerce},
    },
};

impl<'db> Facade<'db> {
    pub fn abi(self, db: &'db dyn Db) -> Abi<'db> {
        Abi::new(db, self)
    }
}

impl<'db> FacadeClass<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiClass<'db> {
        AbiClass::new(db, self)
    }
}

impl<'db> FacadeClassPlain<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiClass<'db> {
        AbiClass::new(db, FacadeClass::Plain(self))
    }

    pub fn abi_coerce(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(RuntimePlainCoerce::new(db, self)).abi(db)
    }
}

impl<'db> FacadeClassDiscriminated<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiClass<'db> {
        AbiClass::new(db, FacadeClass::Discriminated(self))
    }

    pub fn abi_discriminator(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(self.runtime_discriminator(db)).abi(db)
    }
}

impl<'db> FacadeVariant<'db> {
    pub fn abi_coerce(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(RuntimeDiscriminatedCoerce::new(db, self)).abi(db)
    }
}

impl<'db> FacadeFieldPlain<'db> {
    pub fn abi_getter(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(self.getter(db)).abi(db)
    }

    pub fn abi_setter(self, db: &'db dyn Db) -> Option<AbiFunction<'db>> {
        self.setter(db)
            .map(|setter| RuntimeFunction::from(setter).abi(db))
    }
}

impl<'db> FacadeFreeFunction<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(self).abi(db)
    }
}

impl<'db> FacadeFunction<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiFunction<'db> {
        match self {
            Self::Free(function) => function.abi(db),
            Self::Method(method) => method.abi(db),
        }
    }
}

impl<'db> FacadePlainCoerceFunction<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiFunction<'db> {
        self.class(db).abi_coerce(db)
    }
}

impl<'db> FacadeFieldDiscriminator<'db> {
    pub fn abi_getter(self, db: &'db dyn Db) -> AbiFunction<'db> {
        self.class(db).abi_discriminator(db)
    }
}

impl<'db> FacadeMethod<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(self).abi(db)
    }
}

impl<'db> RuntimeFunction<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiFunction<'db> {
        AbiFunction::new(db, self)
    }
}
