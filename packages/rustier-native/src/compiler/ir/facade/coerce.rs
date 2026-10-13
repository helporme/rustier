use crate::compiler::{
    db::Db,
    ir::{
        abi::AbiFunction,
        facade::{FacadeClass, FacadeClassDiscriminated, FacadeClassPlain, FacadeFieldPlain},
    },
};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeCoerce<'db> {
    Plain(FacadePlainCoerceFunction<'db>),
    Discriminated(FacadeDiscriminatedCoerceFunction<'db>),
}

impl<'db> FacadeCoerce<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self {
            Self::Plain(coerce) => coerce.name(db),
            Self::Discriminated(coerce) => coerce.name(db),
        }
    }

    pub fn class(self, db: &'db dyn Db) -> FacadeClass<'db> {
        match self {
            Self::Plain(coerce) => FacadeClass::Plain(coerce.class(db)),
            Self::Discriminated(coerce) => FacadeClass::Discriminated(coerce.class(db)),
        }
    }

    pub fn input_fields(self, db: &'db dyn Db) -> Vec<FacadeFieldPlain<'db>> {
        match self.class(db) {
            FacadeClass::Plain(class) => class.fields(db).collect(),
            FacadeClass::Discriminated(class) => class
                .variants(db)
                .iter()
                .filter(|variant| variant.is_constructible(db))
                .flat_map(|variant| variant.fields(db))
                .collect(),
        }
    }
}

#[salsa::interned(debug)]
pub struct FacadePlainCoerceFunction<'db> {
    pub class: FacadeClassPlain<'db>,
}

impl<'db> FacadePlainCoerceFunction<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        if self
            .class(db)
            .methods(db)
            .any(|method| method.is_static(db) && method.name(db) == "of")
        {
            "$of"
        } else {
            "of"
        }
    }

    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeFieldPlain<'db>> + 'db {
        self.class(db).fields(db)
    }
}

#[salsa::interned(debug)]
pub struct FacadeDiscriminatedCoerceFunction<'db> {
    pub class: FacadeClassDiscriminated<'db>,
}

#[salsa::tracked]
impl<'db> FacadeDiscriminatedCoerceFunction<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        if self
            .class(db)
            .methods(db)
            .any(|method| method.is_static(db) && method.name(db) == "of")
        {
            "$of"
        } else {
            "of"
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn coerce_variants(self, db: &'db dyn Db) -> Vec<CoerceVariant<'db>> {
        self.class(db)
            .variants(db)
            .iter()
            .zip(self.class(db).discriminator(db).variant_names(db))
            .filter(|(variant, _)| variant.is_constructible(db))
            .map(|(variant, name)| CoerceVariant {
                name: name.clone(),
                abi: variant.abi_coerce(db),
                fields: variant.fields(db).collect(),
            })
            .collect()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub struct CoerceVariant<'db> {
    pub name: String,
    pub abi: AbiFunction<'db>,
    pub fields: Vec<FacadeFieldPlain<'db>>,
}

impl<'db> FacadeClass<'db> {
    pub fn coerce(self, db: &'db dyn Db) -> Option<FacadeCoerce<'db>> {
        match self {
            Self::Plain(class) => class.coerce(db).map(FacadeCoerce::Plain),
            Self::Discriminated(class) => class.coerce(db).map(FacadeCoerce::Discriminated),
        }
    }
}

impl<'db> FacadeClassPlain<'db> {
    pub fn coerce(self, db: &'db dyn Db) -> Option<FacadePlainCoerceFunction<'db>> {
        self.is_constructible(db)
            .then(|| FacadePlainCoerceFunction::new(db, self))
    }
}

impl<'db> FacadeClassDiscriminated<'db> {
    pub fn coerce(self, db: &'db dyn Db) -> Option<FacadeDiscriminatedCoerceFunction<'db>> {
        self.is_coercable(db)
            .then(|| FacadeDiscriminatedCoerceFunction::new(db, self))
    }
}
