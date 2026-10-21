use crate::compiler::{
    db::Db,
    ir::{
        facade::{
            Facade, FacadeClassDiscriminated, FacadeClassPlain, FacadeFieldDiscriminator,
            FacadeFieldPlain, FacadeFreeFunction, FacadeFunction, FacadeMethod, FacadeVariant,
            coerce::FacadePlainCoerceFunction,
        },
        runtime::{
            RuntimeDiscriminatedCoerce, RuntimeDropHandle, RuntimeFunction, RuntimePlainCoerce,
            RuntimeType,
        },
        rust::{Type, TypeKind},
        transport::{Transport, TransportFunction, TransportReturn, TransportType, TransportUnion},
    },
};

impl<'db> Facade<'db> {
    pub fn transport(self, db: &'db dyn Db) -> Transport<'db> {
        Transport::new(db, self)
    }
}

impl<'db> RuntimeDropHandle<'db> {
    pub fn transport(self, db: &'db dyn Db) -> TransportFunction<'db> {
        RuntimeFunction::DropHandle(self).transport(db)
    }
}

impl<'db> FacadeClassPlain<'db> {
    pub fn transport_coerce(self, db: &'db dyn Db) -> TransportFunction<'db> {
        RuntimeFunction::from(RuntimePlainCoerce::new(db, self)).transport(db)
    }
}

impl<'db> FacadeClassDiscriminated<'db> {
    pub fn transport_discriminator(self, db: &'db dyn Db) -> TransportFunction<'db> {
        RuntimeFunction::from(self.runtime_discriminator(db)).transport(db)
    }
}

impl<'db> FacadeVariant<'db> {
    pub fn transport_coerce(self, db: &'db dyn Db) -> TransportFunction<'db> {
        RuntimeFunction::from(RuntimeDiscriminatedCoerce::new(db, self)).transport(db)
    }
}

impl<'db> FacadeFieldPlain<'db> {
    pub fn transport_getter(self, db: &'db dyn Db) -> TransportFunction<'db> {
        RuntimeFunction::from(self.getter(db)).transport(db)
    }

    pub fn transport_setter(self, db: &'db dyn Db) -> Option<TransportFunction<'db>> {
        self.setter(db)
            .map(|setter| RuntimeFunction::from(setter).transport(db))
    }
}

impl<'db> FacadeFreeFunction<'db> {
    pub fn transport(self, db: &'db dyn Db) -> TransportFunction<'db> {
        RuntimeFunction::from(self).transport(db)
    }
}

impl<'db> FacadeFunction<'db> {
    pub fn transport(self, db: &'db dyn Db) -> TransportFunction<'db> {
        match self {
            Self::Free(function) => function.transport(db),
            Self::Method(method) => method.transport(db),
        }
    }
}

impl<'db> FacadePlainCoerceFunction<'db> {
    pub fn transport(self, db: &'db dyn Db) -> TransportFunction<'db> {
        self.class(db).transport_coerce(db)
    }
}

impl<'db> FacadeFieldDiscriminator<'db> {
    pub fn transport_getter(self, db: &'db dyn Db) -> TransportFunction<'db> {
        self.class(db).transport_discriminator(db)
    }
}

impl<'db> FacadeMethod<'db> {
    pub fn transport(self, db: &'db dyn Db) -> TransportFunction<'db> {
        RuntimeFunction::from(self).transport(db)
    }
}

impl<'db> RuntimeFunction<'db> {
    pub fn transport(self, db: &'db dyn Db) -> TransportFunction<'db> {
        TransportFunction::new(db, self)
    }
}

impl<'db> RuntimeType<'db> {
    pub fn as_transport_return(self, db: &'db dyn Db) -> TransportReturn<'db> {
        match self {
            Self::Unit => TransportReturn::Value(TransportType::Unit),
            Self::Source(type_) => TransportReturn::Value(type_.transport(db)),
            Self::u32 => TransportReturn::Value(TransportType::f64),
            Self::Tethered(_) => TransportReturn::Value(TransportType::usize),
            Self::Result { ok, err } => TransportReturn::Union(TransportUnion::new(
                db,
                vec![ok.transport(db), err.transport(db)],
            )),
        }
    }
}

impl<'db> Type<'db> {
    pub fn transport(self, db: &'db dyn Db) -> TransportType {
        match self.kind(db) {
            TypeKind::Unit => TransportType::Unit,
            TypeKind::bool => TransportType::bool,
            TypeKind::char
            | TypeKind::i8
            | TypeKind::u8
            | TypeKind::i16
            | TypeKind::u16
            | TypeKind::i32
            | TypeKind::u32
            | TypeKind::f32
            | TypeKind::f64 => TransportType::f64,
            TypeKind::Str | TypeKind::String => TransportType::String,
            TypeKind::JsValue => TransportType::JsValueHandle,
            TypeKind::Borrowed(borrowed) => borrowed.inner(db).transport(db),
            TypeKind::Struct(_) | TypeKind::Enum(_) => TransportType::usize,
            TypeKind::Generic(_) => {
                unreachable!("unsupported Rust types do not cross the ABI")
            }
        }
    }
}
