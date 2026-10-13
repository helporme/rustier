use crate::compiler::{
    db::Db,
    ir::{
        interface::{
            InterClass, InterClassDiscriminated, InterClassPlain, InterCrate, InterField,
            InterModule, InterParentCrate, InterParentModule, Interface,
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
    /// The class representing this type, looking through references, or `None`
    /// if type is not representable as a class.
    pub fn interface_class(self, db: &'db dyn Db) -> Option<InterClass<'db>> {
        match self.kind(db) {
            TypeKind::Struct(struct_) => Some(struct_.interface(db).into()),
            TypeKind::Enum(enum_) => Some(enum_.interface(db).into()),
            TypeKind::Borrowed(borrowed) => borrowed.inner(db).interface_class(db),
            _ => None,
        }
    }
}
