use crate::compiler::{
    db::Db,
    ir::{
        facade::{
            FacadeClass, FacadeClassDiscriminated, FacadeClassPlain, FacadeFieldPlain,
            FacadeVariant,
        },
        transport::TransportFunction,
    },
};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeCoerce<'db> {
    Plain(FacadePlainCoerceFunction<'db>),
    Discriminated(FacadeDiscriminatedCoerceFunction<'db>),
    Option(FacadeOptionCoerceFunction<'db>),
}

impl<'db> FacadeCoerce<'db> {
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

    pub fn class(self, db: &'db dyn Db) -> FacadeClass<'db> {
        match self {
            Self::Plain(coerce) => FacadeClass::Plain(coerce.class(db)),
            Self::Discriminated(coerce) => FacadeClass::Discriminated(coerce.class(db)),
            Self::Option(coerce) => FacadeClass::Discriminated(coerce.class(db)),
        }
    }

    pub fn input_fields(self, db: &'db dyn Db) -> Vec<FacadeFieldPlain<'db>> {
        match self {
            Self::Plain(coerce) => coerce.fields(db).collect(),
            Self::Discriminated(coerce) => coerce
                .class(db)
                .variants(db)
                .iter()
                .filter(|variant| variant.is_constructible(db))
                .flat_map(|variant| variant.fields(db))
                .collect(),
            Self::Option(coerce) => coerce.some(db).fields(db).collect(),
        }
    }
}

#[salsa::interned(debug)]
pub struct FacadePlainCoerceFunction<'db> {
    pub class: FacadeClassPlain<'db>,
}

impl<'db> FacadePlainCoerceFunction<'db> {
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
    #[salsa::tracked(returns(ref))]
    pub fn coerce_variants(self, db: &'db dyn Db) -> Vec<CoerceVariant<'db>> {
        self.class(db)
            .variants(db)
            .iter()
            .zip(self.class(db).discriminator(db).variant_names(db))
            .filter(|(variant, _)| variant.is_constructible(db))
            .map(|(variant, name)| CoerceVariant {
                name: name.clone(),
                abi: variant.transport_coerce(db),
                fields: variant.fields(db).collect(),
            })
            .collect()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub struct CoerceVariant<'db> {
    pub name: String,
    pub abi: TransportFunction<'db>,
    pub fields: Vec<FacadeFieldPlain<'db>>,
}

#[salsa::interned(debug)]
pub struct FacadeOptionCoerceFunction<'db> {
    pub class: FacadeClassDiscriminated<'db>,
    pub some: FacadeVariant<'db>,
    pub none: FacadeVariant<'db>,
}

impl<'db> FacadeClass<'db> {
    pub fn coerce(self, db: &'db dyn Db) -> Option<FacadeCoerce<'db>> {
        match self {
            Self::Plain(class) => class
                .is_constructible(db)
                .then(|| FacadeCoerce::Plain(FacadePlainCoerceFunction::new(db, class))),
            Self::Discriminated(class) => {
                if class.src(db).is_option(db) {
                    let some = class.variants(db).iter().copied().find(|variant| {
                        variant.name(db) == "Some" && variant.is_constructible(db)
                    })?;
                    let none = class.variants(db).iter().copied().find(|variant| {
                        variant.name(db) == "None" && variant.is_constructible(db)
                    })?;

                    Some(FacadeCoerce::Option(FacadeOptionCoerceFunction::new(
                        db, class, some, none,
                    )))
                } else {
                    class.is_coercable(db).then(|| {
                        FacadeCoerce::Discriminated(FacadeDiscriminatedCoerceFunction::new(
                            db, class,
                        ))
                    })
                }
            }
        }
    }
}
