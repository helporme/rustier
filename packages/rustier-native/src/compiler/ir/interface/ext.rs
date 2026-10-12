use crate::compiler::{
    db::Db,
    ir::{
        interface::{
            InterClassDiscriminated, InterClassPlain, InterCrate, InterField, InterModule,
            InterParentCrate, InterParentModule, InterType, Interface,
        },
        rust::{
            Crate, EnumType, FieldQualified, Module, Rust, StructType, Type, TypeKind,
            usages::UsageGraph,
        },
    },
};

impl<'db> Rust<'db> {
    /// The complete public API, reachable from every crate's root module.
    pub fn interface_complete(self, db: &'db dyn Db) -> Interface<'db> {
        self.interface_reachable_from(db, UsageGraph::complete(db))
    }

    pub fn interface_reachable_from(
        self,
        db: &'db dyn Db,
        usages: UsageGraph<'db>,
    ) -> Interface<'db> {
        Interface::new(db, self, usages)
    }
}

impl<'db> Crate<'db> {
    pub fn interface_reachable_from(
        self,
        db: &'db dyn Db,
        usages: UsageGraph<'db>,
    ) -> InterCrate<'db> {
        InterCrate::new(db, self, usages)
    }

    pub fn interface_parent(self, db: &'db dyn Db) -> InterParentCrate<'db> {
        InterParentCrate::new(db, self)
    }
}

impl<'db> Module<'db> {
    pub fn interface_reachable_from(
        self,
        db: &'db dyn Db,
        usages: UsageGraph<'db>,
    ) -> InterModule<'db> {
        InterModule::new(db, self, usages)
    }

    pub fn interface_parent(self, db: &'db dyn Db) -> InterParentModule<'db> {
        InterParentModule::new(db, self)
    }
}

impl<'db> StructType<'db> {
    pub fn interface(self, db: &'db dyn Db) -> InterClassPlain<'db> {
        InterClassPlain::new(db, self)
    }
}

impl<'db> EnumType<'db> {
    pub fn interface(self, db: &'db dyn Db) -> InterClassDiscriminated<'db> {
        InterClassDiscriminated::new(db, self)
    }
}

impl<'db> FieldQualified<'db> {
    pub fn interface(&self, db: &'db dyn Db) -> InterField<'db> {
        self.lower_interface(db)
    }
}

impl<'db> Type<'db> {
    pub fn interface_in(self, db: &'db dyn Db) -> InterType<'db> {
        match self.kind(db) {
            TypeKind::bool => InterType::bool,
            TypeKind::char
            | TypeKind::i8
            | TypeKind::u8
            | TypeKind::i16
            | TypeKind::u16
            | TypeKind::i32
            | TypeKind::u32
            | TypeKind::f32
            | TypeKind::f64 => InterType::f64,
            TypeKind::Str | TypeKind::String => InterType::String,
            TypeKind::Struct(struct_) => InterType::Class {
                is_borrowed: true,
                inner: struct_.interface(db).into(),
            },
            TypeKind::Enum(enum_) => InterType::Class {
                is_borrowed: true,
                inner: enum_.interface(db).into(),
            },
            TypeKind::Borrowed(borrowed) => borrowed.inner(db).interface_in(db),
            TypeKind::Generic(_) => {
                unreachable!("unsupported Rust types do not enter the interface")
            }
        }
    }

    pub fn interface_out(self, db: &'db dyn Db) -> InterType<'db> {
        match self.interface_in(db) {
            InterType::f64 => InterType::f64,
            InterType::String => InterType::String,
            InterType::bool => InterType::bool,
            InterType::Class { inner, .. } => InterType::Class {
                is_borrowed: false,
                inner,
            },
        }
    }
}
