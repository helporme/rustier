use crate::compiler::{
    db::Db,
    ir::{
        facade::{
            Facade, FacadeClass, FacadeClassDiscriminated, FacadeClassPlain, FacadeFieldPlain,
            FacadeModule, FacadeParentModule,
        },
        rust::{
            EnumType, FieldQualified, Module, Rust, StructType, Type, TypeKind, usages::UsageGraph,
        },
    },
};

impl<'db> Rust<'db> {
    pub fn facade_complete(self, db: &'db dyn Db) -> Facade<'db> {
        self.facade_reachable_from(db, UsageGraph::complete(db))
    }

    pub fn facade_reachable_from(self, db: &'db dyn Db, usages: UsageGraph<'db>) -> Facade<'db> {
        Facade::new(db, self, usages)
    }
}

impl<'db> Module<'db> {
    pub fn facade_reachable_from(
        self,
        db: &'db dyn Db,
        usages: UsageGraph<'db>,
    ) -> FacadeModule<'db> {
        FacadeModule::new(db, self, usages)
    }

    pub fn facade_parent(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        FacadeParentModule::new(db, self)
    }
}

impl<'db> StructType<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeClassPlain<'db> {
        FacadeClassPlain::new(db, self)
    }
}

impl<'db> EnumType<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeClassDiscriminated<'db> {
        FacadeClassDiscriminated::new(db, self)
    }
}

impl<'db> FieldQualified<'db> {
    pub fn facade(self, db: &'db dyn Db) -> FacadeFieldPlain<'db> {
        FacadeFieldPlain::new(db, self)
    }
}

impl<'db> Type<'db> {
    pub fn facade_class(self, db: &'db dyn Db) -> Option<FacadeClass<'db>> {
        match self.kind(db) {
            TypeKind::Struct(struct_) => Some(FacadeClass::Plain(struct_.facade(db))),
            TypeKind::Enum(enum_) => Some(FacadeClass::Discriminated(enum_.facade(db))),
            TypeKind::Borrowed(borrowed) => borrowed.inner(db).facade_class(db),
            _ => None,
        }
    }
}
