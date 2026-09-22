use crate::compiler::{
    db::Db,
    ir::{
        rust::{
            Crate, Enum, Field, ImplTarget, Member, Module, Rust, Struct, Type, TypeKind,
            TypeOrigin, usages::UsageGraph,
        },
        shim::{
            Shim, ShimClass, ShimClassDiscriminated, ShimClassPlain, ShimCrate, ShimField,
            ShimFunctionFree, ShimMember, ShimModule, ShimParentCrate, ShimParentModule, ShimType,
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

impl<'db> Member<'db> {
    pub fn shim_reachable_from(self, db: &'db dyn Db, usages: UsageGraph<'db>) -> ShimMember<'db> {
        match self {
            Member::Function(function) => ShimMember::Function(ShimFunctionFree::new(db, function)),
            Member::Struct(struct_) => ShimMember::Class(struct_.shim(db).into()),
            Member::Enum(enum_) => ShimMember::Class(enum_.shim(db).into()),
            Member::Module(module) => ShimMember::Module(module.shim_reachable_from(db, usages)),
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
        Type::new(db, TypeOrigin::from(self)).assert_shim_supported(db);
        match self {
            ImplTarget::Struct(struct_) => struct_.def(db).shim(db).into(),
            ImplTarget::Enum(enum_) => enum_.def(db).shim(db).into(),
        }
    }
}

impl<'db> Field<'db> {
    pub fn shim(&self, db: &'db dyn Db) -> ShimField<'db> {
        ShimField::new(db, self)
    }
}

impl<'db> Type<'db> {
    /// Guard the current nongeneric shim without discarding Rust IR metadata.
    pub(crate) fn assert_shim_supported(self, db: &'db dyn Db) {
        match self.kind(db) {
            TypeKind::Generic(_) => unreachable!("generic types cannot reach the shim"),
            TypeKind::Struct(value) => assert!(
                value.gen_args(db).is_empty() && value.def(db).as_mono_type_DELME(db).is_some(),
                "generic structs cannot reach the shim"
            ),
            TypeKind::Enum(value) => assert!(
                value.gen_args(db).is_empty() && value.def(db).as_mono_type_DELME(db).is_some(),
                "generic enums cannot reach the shim"
            ),
            TypeKind::Borrowed(value) => {
                assert!(
                    value.lifetime(db).is_none(),
                    "explicit lifetimes cannot reach the shim"
                );
                assert!(
                    !matches!(value.inner(db).kind(db), TypeKind::Borrowed(_)),
                    "nested references cannot reach the shim"
                );
                value.inner(db).assert_shim_supported(db);
            }
            TypeKind::bool
            | TypeKind::char
            | TypeKind::i8
            | TypeKind::u8
            | TypeKind::i16
            | TypeKind::u16
            | TypeKind::i32
            | TypeKind::u32
            | TypeKind::f32
            | TypeKind::f64
            | TypeKind::Str
            | TypeKind::String => {}
        }
    }

    pub(super) fn shim_in(self, db: &'db dyn Db) -> ShimType<'db> {
        self.assert_shim_supported(db);
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
                inner: struct_.def(db).shim(db).into(),
            },
            TypeKind::Enum(enum_) => ShimType::Class {
                is_borrowed: true,
                inner: enum_.def(db).shim(db).into(),
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
