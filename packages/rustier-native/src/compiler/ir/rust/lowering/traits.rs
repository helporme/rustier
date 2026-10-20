use ra_ap_hir::HasAttrs as _;

use super::lower_item_span;
use crate::compiler::{
    db::Db,
    ir::rust::{Module, Span, Trait},
};

#[salsa::tracked]
impl<'db> Trait<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_module(self, db: &'db dyn Db) -> Module<'db> {
        Module::new(db, self.hir(db).module(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_name(self, db: &'db dyn Db) -> String {
        let hir = self.hir(db);

        hir.name(db)
            .display(db, hir.module(db).krate(db).edition(db))
            .to_string()
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_docs(self, db: &'db dyn Db) -> Option<String> {
        self.hir(db).hir_docs(db).map(|docs| docs.docs().to_owned())
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_span(self, db: &'db dyn Db) -> Option<Span> {
        lower_item_span(db, self.hir(db))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        compiler::ir::rust::{AsItem as _, Member, Rust},
        test_db,
    };

    #[test]
    fn public_trait_metadata() {
        let db = test_db! {
            /// Reads a value.
            pub trait Read<T> {
                fn read(&self) -> T;
            }

            trait Hidden {}

            #[unstable(feature = "experimental", issue = "none")]
            pub trait Experimental {}
        };

        ra_ap_hir::attach_db(&db, || {
            let root = Rust::get(&db).crates(&db)[0].root(&db);
            let members = root.members(&db);
            let [Member::Trait(trait_)] = members.as_slice() else {
                panic!("only the public stable trait should be lowered: {members:?}");
            };

            assert_eq!(trait_.name(&db), "Read");
            assert_eq!(trait_.module(&db), root);
            assert_eq!(trait_.fqn(&db), "test_crate::Read");
            assert_eq!(trait_.docs(&db).map(String::as_str), Some("Reads a value."));
            assert!(trait_.span(&db).is_some());
        });
    }

    #[test]
    fn same_named_traits_have_distinct_paths() {
        let db = test_db! {
            @file: "models.rs";
            pub mod models {
                pub trait Read {}
            }

            pub trait Read {}
        };

        ra_ap_hir::attach_db(&db, || {
            let root = Rust::get(&db).crates(&db)[0].root(&db);
            let root_trait = root
                .members(&db)
                .iter()
                .find_map(|member| match member {
                    Member::Trait(trait_) => Some(*trait_),
                    _ => None,
                })
                .expect("root trait");
            let models = root.modules(&db).next().expect("models module");
            let [Member::Trait(nested_trait)] = models.members(&db).as_slice() else {
                panic!("models should contain one trait");
            };

            assert_eq!(root_trait.name(&db), nested_trait.name(&db));
            assert_eq!(nested_trait.fqn(&db), "test_crate::models::Read");
            assert_ne!(
                root_trait.path(&db).mangled(&db),
                nested_trait.path(&db).mangled(&db)
            );
            assert_ne!(
                root_trait.span(&db).expect("root span").src_path,
                nested_trait.span(&db).expect("nested span").src_path,
            );
        });
    }

    #[test]
    fn trait_descriptors_do_not_expose_methods_directly() {
        let db = test_db! {
            pub trait Read {
                fn read(&self) -> u32;
            }

            pub struct Counter;

            impl Read for Counter {
                fn read(&self) -> u32 {
                    1
                }
            }
        };

        ra_ap_hir::attach_db(&db, || {
            let facade = Rust::get(&db).facade_complete(&db);
            let root = facade.crates(&db).next().expect("test crate").root(&db);
            let class = root.classes(&db).next().expect("Counter class");

            assert_eq!(root.members(&db).len(), 2);
            assert_eq!(class.name(&db), "Counter");
            assert_eq!(class.methods(&db).count(), 0);
        });
    }
}
