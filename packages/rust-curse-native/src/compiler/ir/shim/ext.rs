use crate::compiler::{
    db::Db,
    ir::{
        rust::{Enum, Field, ImplTarget, Mod, Struct, Type, ValueType},
        shim::{ShimClass, ShimClassSource, ShimField, ShimMod, ShimType},
    },
};

impl<'db> Mod<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimMod<'db> {
        ShimMod::new(db, self)
    }
}

impl<'db> Struct<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimClass<'db> {
        ShimClass::new(db, ShimClassSource::Struct(self))
    }
}

impl<'db> Enum<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimClass<'db> {
        ShimClass::new(db, ShimClassSource::Enum(self))
    }
}

impl<'db> ImplTarget<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimClass<'db> {
        match self {
            ImplTarget::Struct(struct_) => struct_.shim(db),
            ImplTarget::Enum(enum_) => enum_.shim(db),
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
                inner: struct_.shim(db),
            },
            ValueType::Enum(enum_) => ShimType::Class {
                is_borrowed: true,
                inner: enum_.shim(db),
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
