use crate::compiler::{
    db::Db,
    ir::{
        rust::{Crate, Enum, Field, ImplTarget, Member, Module, Rust, Struct, Type, ValueType},
        shim::{
            Shim, ShimClass, ShimClassDiscriminated, ShimClassPlain, ShimCrate, ShimField,
            ShimFunctionFree, ShimMember, ShimModule, ShimType,
        },
    },
};

impl<'db> Rust<'db> {
    pub fn shim(self) -> Shim<'db> {
        Shim { rust: self }
    }
}

impl<'db> Crate<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimCrate<'db> {
        ShimCrate::new(db, self)
    }
}

impl<'db> Module<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimModule<'db> {
        ShimModule::new(db, self)
    }
}

impl<'db> Member<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimMember<'db> {
        match self {
            Member::Function(function) => ShimMember::Function(ShimFunctionFree::new(db, function)),
            Member::Struct(struct_) => ShimMember::Class(struct_.shim(db).into()),
            Member::Enum(enum_) => ShimMember::Class(enum_.shim(db).into()),
            Member::Module(module) => ShimMember::Module(module.shim(db)),
        }
    }
}

impl<'db> Struct<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimClassPlain<'db> {
        ShimClassPlain::new(db, self)
    }
}

impl<'db> Enum<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimClassDiscriminated<'db> {
        ShimClassDiscriminated::new(db, self)
    }
}

impl<'db> ImplTarget<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimClass<'db> {
        match self {
            ImplTarget::Struct(struct_) => struct_.shim(db).into(),
            ImplTarget::Enum(enum_) => enum_.shim(db).into(),
        }
    }
}

impl<'db> Field<'db> {
    pub fn shim(&self, db: &'db dyn Db) -> ShimField<'db> {
        ShimField::new(db, self)
    }
}

impl<'db> Type<'db> {
    pub(super) fn shim_in(self, db: &'db dyn Db) -> ShimType<'db> {
        match self.deborrow() {
            ValueType::bool => ShimType::bool,
            ValueType::char
            | ValueType::i8
            | ValueType::u8
            | ValueType::i16
            | ValueType::u16
            | ValueType::i32
            | ValueType::u32
            | ValueType::f32
            | ValueType::f64 => ShimType::f64,
            ValueType::Str | ValueType::String => ShimType::String,
            ValueType::Struct(struct_) => ShimType::Class {
                is_borrowed: true,
                inner: struct_.shim(db).into(),
            },
            ValueType::Enum(enum_) => ShimType::Class {
                is_borrowed: true,
                inner: enum_.shim(db).into(),
            },
        }
    }

    pub(super) fn shim_out(self, db: &'db dyn Db) -> ShimType<'db> {
        match self.shim_in(db) {
            ShimType::f64 => ShimType::f64,
            ShimType::String => ShimType::String,
            ShimType::bool => ShimType::bool,
            ShimType::Class { inner, .. } => ShimType::Class {
                is_borrowed: false,
                inner,
            },
        }
    }
}
