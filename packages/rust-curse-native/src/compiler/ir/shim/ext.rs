use crate::compiler::{
    db::Db,
    ir::{
        rust::{
            Crate, EnumType, FieldQualified, Module, Rust, StructType, Type, TypeKind,
            usages::UsageGraph,
        },
        shim::{
            Shim, ShimClassDiscriminated, ShimClassPlain, ShimCrate, ShimField, ShimModule,
            ShimParentCrate, ShimParentModule, ShimType,
        },
    },
};

impl<'db> Rust<'db> {
    /// The complete public API, reachable from every crate's root module.
    pub fn shim_complete(self, db: &'db dyn Db) -> Shim<'db> {
        self.shim_reachable_from(db, UsageGraph::complete(db))
    }

    pub fn shim_reachable_from(self, db: &'db dyn Db, usages: UsageGraph<'db>) -> Shim<'db> {
        Shim::new(db, self, usages)
    }
}

impl<'db> Crate<'db> {
    pub fn shim_reachable_from(self, db: &'db dyn Db, usages: UsageGraph<'db>) -> ShimCrate<'db> {
        ShimCrate::new(db, self, usages)
    }

    pub fn shim_parent(self, db: &'db dyn Db) -> ShimParentCrate<'db> {
        ShimParentCrate::new(db, self)
    }
}

impl<'db> Module<'db> {
    pub fn shim_reachable_from(self, db: &'db dyn Db, usages: UsageGraph<'db>) -> ShimModule<'db> {
        ShimModule::new(db, self, usages)
    }

    pub fn shim_parent(self, db: &'db dyn Db) -> ShimParentModule<'db> {
        ShimParentModule::new(db, self)
    }
}

impl<'db> StructType<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimClassPlain<'db> {
        ShimClassPlain::new(db, self)
    }
}

impl<'db> EnumType<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimClassDiscriminated<'db> {
        ShimClassDiscriminated::new(db, self)
    }
}

impl<'db> FieldQualified<'db> {
    pub fn shim(&self, db: &'db dyn Db) -> ShimField<'db> {
        self.lower_shim(db)
    }
}

impl<'db> Type<'db> {
    pub(super) fn shim_in(self, db: &'db dyn Db) -> ShimType<'db> {
        match self.kind(db) {
            TypeKind::bool => ShimType::bool,
            TypeKind::char
            | TypeKind::i8
            | TypeKind::u8
            | TypeKind::i16
            | TypeKind::u16
            | TypeKind::i32
            | TypeKind::u32
            | TypeKind::f32
            | TypeKind::f64 => ShimType::f64,
            TypeKind::Str | TypeKind::String => ShimType::String,
            TypeKind::Struct(struct_) => ShimType::Class {
                is_borrowed: true,
                inner: struct_.shim(db).into(),
            },
            TypeKind::Enum(enum_) => ShimType::Class {
                is_borrowed: true,
                inner: enum_.shim(db).into(),
            },
            TypeKind::Borrowed(borrowed) => borrowed.inner(db).shim_in(db),
            TypeKind::Generic(_) => unreachable!("unsupported Rust types do not enter the shim"),
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
