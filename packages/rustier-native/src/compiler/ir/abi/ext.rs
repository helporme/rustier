use crate::compiler::{
    db::Db,
    ir::{
        abi::{Abi, AbiFunction, AbiReturn, AbiType, AbiUnion},
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
    },
};

impl<'db> Facade<'db> {
    pub fn abi(self, db: &'db dyn Db) -> Abi<'db> {
        Abi::new(db, self)
    }
}

impl<'db> RuntimeDropHandle<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::DropHandle(self).abi(db)
    }
}

impl<'db> FacadeClassPlain<'db> {
    pub fn abi_coerce(self, db: &'db dyn Db) -> AbiFunction<'db> {
        RuntimeFunction::from(RuntimePlainCoerce::new(db, self)).abi(db)
    }
}

impl<'db> FacadeClassDiscriminated<'db> {
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

    pub fn abi_ret(self, db: &'db dyn Db) -> AbiReturn<'db> {
        let value = self.ret_type(db).abi(db);

        match self {
            Self::DropHandle(_) => AbiReturn::Value(value),
            _ => AbiReturn::Union(AbiUnion::new(db, vec![value, AbiType::String])),
        }
    }
}

impl<'db> RuntimeType<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiType {
        match self {
            Self::Unit => AbiType::Unit,
            Self::Source(type_) => type_.abi(db),
            Self::u32 => AbiType::f64,
            Self::Tethered(_) => AbiType::usize,
        }
    }
}

impl<'db> Type<'db> {
    pub fn abi(self, db: &'db dyn Db) -> AbiType {
        match self.kind(db) {
            TypeKind::Unit => AbiType::Unit,
            TypeKind::bool => AbiType::bool,
            TypeKind::char
            | TypeKind::i8
            | TypeKind::u8
            | TypeKind::i16
            | TypeKind::u16
            | TypeKind::i32
            | TypeKind::u32
            | TypeKind::f32
            | TypeKind::f64 => AbiType::f64,
            TypeKind::Str | TypeKind::String => AbiType::String,
            TypeKind::Borrowed(borrowed) => borrowed.inner(db).abi(db),
            TypeKind::Struct(_) | TypeKind::Enum(_) => AbiType::usize,
            TypeKind::Generic(_) => {
                unreachable!("unsupported Rust types do not cross the ABI")
            }
        }
    }
}
