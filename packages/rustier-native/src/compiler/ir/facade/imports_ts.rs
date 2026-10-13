use std::collections::HashSet;

use itertools::Either;

use crate::compiler::{
    db::Db,
    ir::facade::{FacadeClass, FacadeClassTemplate, FacadeFunction, FacadeMember, FacadeModule},
};

#[salsa::tracked(debug)]
pub struct FacadeImportTs<'db> {
    pub originating_from: FacadeModule<'db>,
    pub class: FacadeClass<'db>,
}

#[salsa::tracked]
impl<'db> FacadeModule<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn decl_imports(self, db: &'db dyn Db) -> Vec<FacadeImportTs<'db>> {
        let mut classes = HashSet::new();

        for member in self.members(db) {
            match member {
                FacadeMember::Function(function) => {
                    classes.extend(function.as_fn().decl_imports(db))
                }
                FacadeMember::Class(class) => classes.extend(class.decl_imports(db)),
                FacadeMember::ClassTemplate(template) => classes.extend(template.decl_imports(db)),
                FacadeMember::Module(_) => {}
            }
        }

        let mut imports = classes
            .into_iter()
            .filter(|class| class.module(db) != self.as_parent(db))
            .map(|class| FacadeImportTs::new(db, self, class))
            .collect::<Vec<_>>();

        imports.sort_by_key(|import| {
            let class = import.class(db);

            (
                class.type_name(db).name(db),
                class.module(db).virtual_path(db),
            )
        });
        imports.dedup_by(|left, right| {
            left.class(db).module(db) == right.class(db).module(db)
                && left.class(db).type_name(db).name(db) == right.class(db).type_name(db).name(db)
        });

        imports
    }
}

impl<'db> FacadeClassTemplate<'db> {
    fn decl_imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeClass<'db>> + 'db {
        self.mappings(db)
            .iter()
            .flat_map(move |(_, class)| class.decl_imports(db))
    }
}

impl<'db> FacadeFunction<'db> {
    fn decl_imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeClass<'db>> + 'db {
        self.params(db)
            .flat_map(move |param| param.type_ts(db).decl_imports(db))
            .chain(
                self.ret_type_ts(db)
                    .into_iter()
                    .flat_map(move |type_| type_.decl_imports(db)),
            )
    }
}

impl<'db> FacadeClass<'db> {
    fn decl_imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeClass<'db>> + 'db {
        match self {
            Self::Plain(class) => Either::Left(
                class
                    .fields(db)
                    .flat_map(move |field| field.get_type_ts(db).decl_imports(db))
                    .chain(
                        class
                            .methods(db)
                            .flat_map(move |method| method.as_fn().decl_imports(db)),
                    )
                    .chain(self.type_name(db).decl_imports(db).iter().copied()),
            ),
            Self::Discriminated(class) => Either::Right(
                class
                    .variants(db)
                    .iter()
                    .flat_map(move |variant| {
                        variant
                            .fields(db)
                            .flat_map(move |field| field.get_type_ts(db).decl_imports(db))
                    })
                    .chain(
                        class
                            .methods(db)
                            .flat_map(move |method| method.as_fn().decl_imports(db)),
                    )
                    .chain(self.type_name(db).decl_imports(db).iter().copied()),
            ),
        }
    }
}
