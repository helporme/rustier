use crate::compiler::{
    db::Db,
    ir::{
        facade::{
            Facade, FacadeClass, FacadeClassDiscriminated, FacadeClassPlain, FacadeFieldPlain,
            FacadeVariant,
        },
        runtime::{
            Runtime, RuntimeClass, RuntimeDiscriminatedCoerce, RuntimeDiscriminator, RuntimeGetter,
            RuntimePlainCoerce, RuntimeSetter,
            tethering::{RuntimeTetheredField, RuntimeTethering},
        },
        rust::Type,
    },
};

impl<'db> Facade<'db> {
    pub fn runtime(self, db: &'db dyn Db) -> Runtime<'db> {
        Runtime::new(db, self)
    }
}

impl<'db> FacadeClass<'db> {
    pub fn runtime(self, db: &'db dyn Db) -> RuntimeClass<'db> {
        RuntimeClass::new(db, self)
    }

    pub fn tethering(self, db: &'db dyn Db) -> RuntimeTethering<'db> {
        RuntimeTethering::new(db, self)
    }
}

impl<'db> Type<'db> {
    pub fn tethering(self, db: &'db dyn Db) -> Option<RuntimeTethering<'db>> {
        self.facade_class(db).map(|class| class.tethering(db))
    }
}

impl<'db> FacadeFieldPlain<'db> {
    pub fn tethered(self, db: &'db dyn Db) -> RuntimeTetheredField<'db> {
        RuntimeTetheredField::new(db, self.src(db))
    }

    pub fn getter(self, db: &'db dyn Db) -> RuntimeGetter<'db> {
        RuntimeGetter::new(db, self)
    }

    pub fn setter(self, db: &'db dyn Db) -> Option<RuntimeSetter<'db>> {
        (!self.is_readonly(db)).then(|| RuntimeSetter::new(db, self))
    }
}

impl<'db> FacadeClassPlain<'db> {
    pub fn runtime_coerce(self, db: &'db dyn Db) -> Option<RuntimePlainCoerce<'db>> {
        self.is_constructible(db)
            .then(|| RuntimePlainCoerce::new(db, self))
    }
}

impl<'db> FacadeClassDiscriminated<'db> {
    pub fn runtime_discriminator(self, db: &'db dyn Db) -> RuntimeDiscriminator<'db> {
        RuntimeDiscriminator::new(db, self)
    }
}

impl<'db> FacadeVariant<'db> {
    pub fn runtime_coerce(self, db: &'db dyn Db) -> Option<RuntimeDiscriminatedCoerce<'db>> {
        self.is_constructible(db)
            .then(|| RuntimeDiscriminatedCoerce::new(db, self))
    }
}
