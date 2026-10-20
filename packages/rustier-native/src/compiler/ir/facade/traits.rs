use std::collections::BTreeMap;

use itertools::Itertools;

use crate::compiler::{
    db::Db,
    ir::{
        facade::{
            Facade, FacadeClass, FacadeGeneric, FacadeMethod, FacadeModule, FacadeParentModule,
        },
        rust::{GenArg, ImplQualified, ImplTrait, Trait, TypeKind, usages::UsageGraph},
    },
};

#[salsa::interned(debug)]
pub struct FacadeTrait<'db> {
    pub src: Trait<'db>,
    pub usages: UsageGraph<'db>,
}

#[salsa::tracked]
impl<'db> FacadeTrait<'db> {
    pub fn module(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        self.src(db).module(db).facade_parent(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        let name = self.src(db).name(db);

        name.strip_prefix("r#").unwrap_or(name)
    }

    pub fn views(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeTraitView<'db>> + 'db {
        self.usages(db)
            .facade_trait_views(db)
            .iter()
            .copied()
            .filter(move |view| view.src(db).trait_(db) == self.src(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn specializations(self, db: &'db dyn Db) -> Vec<FacadeTraitSpecialization<'db>> {
        let mut mappings = BTreeMap::new();

        for view in self.views(db) {
            let identity = view.src(db).trait_path(db).mangled(db);
            let entry = mappings
                .entry(identity)
                .or_insert_with(|| FacadeTraitSpecialization {
                    arguments: view.arguments(db).clone(),
                    views: Vec::new(),
                });

            entry.views.push(view);
        }

        mappings.into_values().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct FacadeTraitSpecialization<'db> {
    pub arguments: Vec<FacadeTraitArgument<'db>>,
    pub views: Vec<FacadeTraitView<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum FacadeTraitArgument<'db> {
    Value(FacadeGeneric<'db>),
    Reference {
        prefix: String,
        class: FacadeClass<'db>,
    },
}

#[salsa::interned(debug)]
pub struct FacadeTraitView<'db> {
    pub class: FacadeClass<'db>,
    pub src: ImplTrait<'db>,
}

#[salsa::tracked]
impl<'db> FacadeTraitView<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn arguments(self, db: &'db dyn Db) -> Vec<FacadeTraitArgument<'db>> {
        self.src(db)
            .trait_args(db)
            .iter()
            .map(|GenArg::Type(type_)| {
                let mut inner = *type_;
                let mut prefix = String::new();

                while let TypeKind::Borrowed(reference) = inner.kind(db) {
                    prefix.push('&');
                    if reference.is_mutable(db) {
                        prefix.push_str("mut ");
                    }

                    inner = reference.inner(db);
                }

                match inner.facade_class(db) {
                    Some(class) if prefix.is_empty() => {
                        FacadeTraitArgument::Value(FacadeGeneric::Class(class))
                    }
                    Some(class) => FacadeTraitArgument::Reference {
                        prefix: prefix.trim_end().to_owned(),
                        class,
                    },
                    None => FacadeTraitArgument::Value(FacadeGeneric::Symbol(
                        type_.display(db).to_owned(),
                    )),
                }
            })
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub fn methods(self, db: &'db dyn Db) -> Vec<FacadeMethod<'db>> {
        self.src(db)
            .functions(db)
            .filter_map(|function| function.as_callable(db))
            .map(|callable| FacadeMethod::new(db, callable))
            .collect()
    }

    pub fn static_methods(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeMethod<'db>> + 'db {
        self.methods(db)
            .iter()
            .copied()
            .filter(move |method| method.is_static(db))
    }

    pub fn instance_methods(
        self,
        db: &'db dyn Db,
    ) -> impl Iterator<Item = FacadeMethod<'db>> + 'db {
        self.methods(db)
            .iter()
            .copied()
            .filter(move |method| !method.is_static(db))
    }
}

#[salsa::tracked]
impl<'db> FacadeClass<'db> {
    pub fn all_methods(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeMethod<'db>> + 'db {
        let trait_methods = self
            .trait_views(db)
            .iter()
            .flat_map(|view| view.methods(db).iter().copied());

        self.methods(db).chain(trait_methods)
    }

    #[salsa::tracked(returns(ref))]
    pub fn trait_views(self, db: &'db dyn Db) -> Vec<FacadeTraitView<'db>> {
        self.src_type(db)
            .impls(db)
            .iter()
            .filter_map(|impl_| {
                let ImplQualified::Trait(impl_) = impl_ else {
                    return None;
                };
                let view = FacadeTraitView::new(db, self, *impl_);

                (!view.methods(db).is_empty()).then_some(view)
            })
            .sorted_by_key(|view| view.src(db).trait_path(db).mangled(db))
            .collect()
    }
}

#[salsa::tracked]
impl<'db> UsageGraph<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn facade_trait_views(self, db: &'db dyn Db) -> Vec<FacadeTraitView<'db>> {
        self.types(db)
            .filter_map(|type_| match type_.kind(db) {
                TypeKind::Struct(type_) => Some(FacadeClass::Plain(type_.facade(db))),
                TypeKind::Enum(type_) => Some(FacadeClass::Discriminated(type_.facade(db))),
                _ => None,
            })
            .flat_map(|class| class.trait_views(db).iter().copied())
            .sorted_by_key(|view| {
                (
                    view.class(db).module(db).virtual_path(db),
                    view.class(db).name(db),
                    view.src(db).trait_path(db).mangled(db),
                )
            })
            .collect()
    }
}

#[salsa::tracked]
impl<'db> Facade<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn traits(self, db: &'db dyn Db) -> Vec<FacadeTrait<'db>> {
        self.usages(db)
            .facade_trait_views(db)
            .iter()
            .map(|view| view.src(db).trait_(db))
            .unique()
            .map(|src| FacadeTrait::new(db, src, self.usages(db)))
            .sorted_by_key(|trait_| trait_.src(db).path(db).fqn(db))
            .collect()
    }
}

#[salsa::tracked]
impl<'db> FacadeModule<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn traits(self, db: &'db dyn Db) -> Vec<FacadeTrait<'db>> {
        self.usages(db)
            .facade_trait_views(db)
            .iter()
            .map(|view| view.src(db).trait_(db))
            .filter(|trait_| trait_.module(db) == self.src(db))
            .unique()
            .map(|src| FacadeTrait::new(db, src, self.usages(db)))
            .sorted_by_key(|trait_| trait_.name(db))
            .collect()
    }
}
