use crate::compiler::{
    db::Db,
    ir::{
        interface::{
            InterClass, InterClassDiscriminated, InterClassDiscriminatedVariant, InterClassPlain,
            InterField, Interface,
        },
        runtime::{
            Runtime, RuntimeClass, RuntimeDiscriminatedCoerce, RuntimeDiscriminator, RuntimeGetter,
            RuntimePlainCoerce, RuntimeSetter,
        },
    },
};

impl<'db> Interface<'db> {
    pub fn runtime(self, db: &'db dyn Db) -> Runtime<'db> {
        Runtime::new(db, self)
    }
}

impl<'db> InterClass<'db> {
    pub fn runtime(self, db: &'db dyn Db) -> RuntimeClass<'db> {
        RuntimeClass::new(db, self)
    }
}

impl<'db> InterField<'db> {
    pub fn getter(self, db: &'db dyn Db) -> RuntimeGetter<'db> {
        RuntimeGetter::new(db, self)
    }

    pub fn setter(self, db: &'db dyn Db) -> Option<RuntimeSetter<'db>> {
        (!self.is_readonly(db)).then(|| RuntimeSetter::new(db, self))
    }
}

impl<'db> InterClassPlain<'db> {
    pub fn coerce(self, db: &'db dyn Db) -> Option<RuntimePlainCoerce<'db>> {
        self.is_constructible(db)
            .then(|| RuntimePlainCoerce::new(db, self))
    }
}

impl<'db> InterClassDiscriminated<'db> {
    pub fn discriminator(self, db: &'db dyn Db) -> RuntimeDiscriminator<'db> {
        RuntimeDiscriminator::new(db, self)
    }
}

impl<'db> InterClassDiscriminatedVariant<'db> {
    pub fn coerce(self, db: &'db dyn Db) -> Option<RuntimeDiscriminatedCoerce<'db>> {
        self.is_constructible(db)
            .then(|| RuntimeDiscriminatedCoerce::new(db, self))
    }
}
