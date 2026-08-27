use std::collections::HashSet;

use ra_ap_hir::{
    Adt, AssocItem, GenericDef, HasSource as _, HasVisibility as _, Module, ModuleDef,
    Type as HirType, Visibility,
};

use super::{is_clonable, lower_type};
use crate::compiler::{
    CrateApiDiscovery,
    db::Db,
    inputs::CratesMetadata,
    ir::rust::{Crate, Enum, Mod, Struct, Type, ValueType},
};

#[salsa::tracked(returns(ref))]
fn calc_api_reachability<'db>(db: &'db dyn Db, target_crate: Crate<'db>) -> HashSet<Adt> {
    let mut reachability = ApiReachability {
        db,
        target_crate,
        visited_adts: HashSet::new(),
        reachable_adts: HashSet::new(),
    };

    let metadata = CratesMetadata::get(db);
    for hir_crate in ra_ap_hir::Crate::all(db) {
        let crate_ = Crate::new(db, hir_crate);
        if metadata
            .crates
            .get(crate_.root_file(db))
            .is_some_and(|metadata| metadata.api_discovery == CrateApiDiscovery::Eager)
        {
            reachability.visit_mod(crate_.hir(db).root_module(db));
        }
    }

    reachability.reachable_adts
}

struct ApiReachability<'db> {
    db: &'db dyn Db,
    target_crate: Crate<'db>,
    visited_adts: HashSet<Adt>,
    reachable_adts: HashSet<Adt>,
}

impl<'db> ApiReachability<'db> {
    fn visit_mod(&mut self, mod_: Module) {
        for declaration in mod_.declarations(self.db) {
            if declaration.visibility(self.db) != Visibility::Public {
                continue;
            }
            match declaration {
                ModuleDef::Module(mod_) => self.visit_mod(mod_),
                ModuleDef::Function(function) => self.visit_function(function),
                ModuleDef::Adt(adt) if GenericDef::Adt(adt).params(self.db).is_empty() => {
                    self.visit_adt(adt);
                }
                _ => {}
            }
        }
    }

    fn visit_adt(&mut self, adt: Adt) {
        if !self.visited_adts.insert(adt) {
            return;
        }

        let crate_ = Crate::new(self.db, adt.module(self.db).krate(self.db));
        if crate_ == self.target_crate && crate_.api_discovery(self.db) == CrateApiDiscovery::Lazy {
            self.reachable_adts.insert(adt);
        }

        match adt {
            Adt::Struct(struct_) => {
                for field in struct_.fields(self.db) {
                    if field.visibility(self.db) == Visibility::Public {
                        let type_ = ra_ap_hir::attach_db(self.db, || field.ty(self.db));
                        self.visit_type(&type_, crate_.hir(self.db));
                    }
                }
            }
            Adt::Enum(enum_) => {
                for variant in enum_.variants(self.db) {
                    for field in variant.fields(self.db) {
                        let type_ = ra_ap_hir::attach_db(self.db, || field.ty(self.db));
                        self.visit_type(&type_, crate_.hir(self.db));
                    }
                }
            }
            Adt::Union(_) => return,
        }

        let type_ = ra_ap_hir::attach_db(self.db, || match adt {
            Adt::Struct(struct_) => struct_.ty(self.db),
            Adt::Enum(enum_) => enum_.ty(self.db),
            Adt::Union(_) => unreachable!(),
        });
        for impl_ in ra_ap_hir::attach_db(self.db, || ra_ap_hir::Impl::all_for_type(self.db, type_))
        {
            if impl_.trait_(self.db).is_some()
                || !GenericDef::Impl(impl_).params(self.db).is_empty()
                || impl_.is_unsafe(self.db)
                || impl_.is_negative(self.db)
                || impl_.source(self.db).is_none()
            {
                continue;
            }
            for item in impl_.items(self.db) {
                if let AssocItem::Function(function) = item
                    && function.visibility(self.db) == Visibility::Public
                {
                    self.visit_function(function);
                }
            }
        }
    }

    fn visit_function(&mut self, function: ra_ap_hir::Function) {
        if function.is_async(self.db)
            || function.is_unsafe(self.db)
            || function.is_varargs(self.db)
            || !GenericDef::Function(function).params(self.db).is_empty()
        {
            return;
        }

        let crate_ = function.module(self.db).krate(self.db);
        let params = ra_ap_hir::attach_db(self.db, || function.params_without_self(self.db));
        let mut referenced_types = Vec::with_capacity(params.len() + 1);
        for param in params {
            let type_ = param.ty();
            let Ok(lowered) = lower_type(self.db, type_, crate_) else {
                return;
            };
            if matches!(
                lowered,
                Type::Value(ValueType::Struct(_) | ValueType::Enum(_))
            ) && !is_clonable(self.db, type_, crate_)
            {
                return;
            }
            referenced_types.push(lowered.deborrow());
        }

        let return_type = ra_ap_hir::attach_db(self.db, || function.ret_type(self.db));
        if !return_type.is_unit() {
            let Ok(Type::Value(return_type)) = lower_type(self.db, &return_type, crate_) else {
                return;
            };
            referenced_types.push(return_type);
        }

        if let Some(receiver) = function.self_param(self.db)
            && receiver.access(self.db) == ra_ap_hir::Access::Owned
        {
            let receiver_type = ra_ap_hir::attach_db(self.db, || receiver.ty(self.db));
            if !is_clonable(self.db, &receiver_type, crate_) {
                return;
            }
        }

        for type_ in referenced_types {
            self.visit_value_type(type_);
        }
    }

    fn visit_type(&mut self, type_: &HirType<'db>, crate_: ra_ap_hir::Crate) {
        if let Ok(type_) = lower_type(self.db, type_, crate_) {
            self.visit_value_type(type_.deborrow());
        }
    }

    fn visit_value_type(&mut self, type_: ValueType<'db>) {
        match type_ {
            ValueType::Struct(struct_) => self.visit_adt(Adt::Struct(struct_.hir(self.db))),
            ValueType::Enum(enum_) => self.visit_adt(Adt::Enum(enum_.hir(self.db))),
            _ => {}
        }
    }
}

pub(super) trait ReachableItem: Copy {
    fn is_reachable(self, db: &dyn Db) -> bool;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
enum ReachableAdt<'db> {
    Struct(Struct<'db>),
    Enum(Enum<'db>),
}

impl ReachableItem for Adt {
    fn is_reachable(self, db: &dyn Db) -> bool {
        match self {
            Adt::Struct(struct_) => {
                is_adt_reachable(db, ReachableAdt::Struct(Struct::new(db, struct_)))
            }
            Adt::Enum(enum_) => is_adt_reachable(db, ReachableAdt::Enum(Enum::new(db, enum_))),
            Adt::Union(_) => false,
        }
    }
}

impl ReachableItem for Module {
    fn is_reachable(self, db: &dyn Db) -> bool {
        is_module_reachable(db, Mod::new(db, self))
    }
}

impl ReachableItem for ra_ap_hir::Crate {
    fn is_reachable(self, db: &dyn Db) -> bool {
        is_crate_reachable(db, Crate::new(db, self))
    }
}

#[salsa::tracked]
fn is_adt_reachable<'db>(db: &'db dyn Db, adt: ReachableAdt<'db>) -> bool {
    let (hir, crate_) = match adt {
        ReachableAdt::Struct(struct_) => {
            let hir = struct_.hir(db);
            (Adt::Struct(hir), Crate::new(db, hir.module(db).krate(db)))
        }
        ReachableAdt::Enum(enum_) => {
            let hir = enum_.hir(db);
            (Adt::Enum(hir), Crate::new(db, hir.module(db).krate(db)))
        }
    };
    if !GenericDef::Adt(hir).params(db).is_empty() {
        return false;
    }

    match crate_.api_discovery(db) {
        CrateApiDiscovery::Eager => true,
        CrateApiDiscovery::Lazy => calc_api_reachability(db, crate_).contains(&hir),
    }
}

#[salsa::tracked]
fn is_module_reachable<'db>(db: &'db dyn Db, mod_: Mod<'db>) -> bool {
    let hir = mod_.hir(db);
    let crate_ = Crate::new(db, hir.krate(db));
    let include_functions = crate_.api_discovery(db) == CrateApiDiscovery::Eager;
    hir.declarations(db)
        .into_iter()
        .filter(|declaration| declaration.visibility(db) == Visibility::Public)
        .any(|declaration| match declaration {
            ModuleDef::Module(mod_) => mod_.is_reachable(db),
            ModuleDef::Function(_) => include_functions,
            ModuleDef::Adt(adt) => adt.is_reachable(db),
            _ => false,
        })
}

#[salsa::tracked]
fn is_crate_reachable<'db>(db: &'db dyn Db, crate_: Crate<'db>) -> bool {
    match crate_.api_discovery(db) {
        CrateApiDiscovery::Eager => true,
        CrateApiDiscovery::Lazy => !calc_api_reachability(db, crate_).is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use ra_ap_test_fixture::ChangeFixture;

    use super::*;
    use crate::compiler::{
        CrateApiImportMode, FilePath, Platform,
        db::RootDatabase,
        inputs::{CrateMetadata, CratesMetadata},
        ir::rust::{Member, Mod, Rust},
    };

    #[test]
    fn lowers_complete_eager_api_and_only_reachable_lazy_adts() {
        let fixture = r#"
//- /eager/lib.rs crate:eager deps:lazy edition:2024
pub struct EagerOnly {
    pub value: lazy::UsedEnum,
}

pub fn expose(value: &lazy::Used) -> lazy::Used {
    loop {}
}

//- /lazy/lib.rs crate:lazy edition:2024 new_source_root:library
pub struct Used {
    pub nested: nested::Nested,
}

impl Used {
    pub fn method_type(&self) -> MethodType {
        loop {}
    }
}

pub mod nested {
    pub struct Nested {
        pub value: i32,
    }
}

pub struct MethodType {
    pub value: i32,
}

pub enum UsedEnum {
    Item(nested::Nested),
}

pub struct Unused {
    pub value: i32,
}

pub enum UnusedEnum {
    Item,
}

pub fn unused_function() {}

//- /unused_lazy/lib.rs crate:unused_lazy edition:2024 new_source_root:library
pub struct NeverReferenced {
    pub value: i32,
}
"#;
        let eager_root = FilePath::new_virtual("/eager/lib.rs").unwrap();
        let lazy_root = FilePath::new_virtual("/lazy/lib.rs").unwrap();
        let unused_lazy_root = FilePath::new_virtual("/unused_lazy/lib.rs").unwrap();
        let mut db = RootDatabase::default();
        db.apply_change(ChangeFixture::parse(fixture).change);
        CratesMetadata::set(
            &mut db,
            CratesMetadata {
                crates: HashMap::from([
                    (
                        eager_root,
                        CrateMetadata {
                            api_discovery: CrateApiDiscovery::Eager,
                            api_import_mode: CrateApiImportMode::Mod,
                        },
                    ),
                    (
                        lazy_root,
                        CrateMetadata {
                            api_discovery: CrateApiDiscovery::Lazy,
                            api_import_mode: CrateApiImportMode::Mod,
                        },
                    ),
                    (
                        unused_lazy_root,
                        CrateMetadata {
                            api_discovery: CrateApiDiscovery::Lazy,
                            api_import_mode: CrateApiImportMode::Mod,
                        },
                    ),
                ]),
            },
        );
        Platform::set(&mut db, Platform::Node);

        let rust = Rust::get(&db);
        let eager = rust
            .crates(&db)
            .iter()
            .find(|crate_| crate_.name(&db) == "eager")
            .unwrap();
        let lazy = rust
            .crates(&db)
            .iter()
            .find(|crate_| crate_.name(&db) == "lazy")
            .unwrap();

        assert_eq!(
            member_labels(&db, eager.root(&db)),
            HashSet::from(["fn expose".to_owned(), "struct EagerOnly".to_owned(),])
        );
        assert_eq!(
            member_labels(&db, lazy.root(&db)),
            HashSet::from([
                "enum UsedEnum".to_owned(),
                "mod nested".to_owned(),
                "struct MethodType".to_owned(),
                "struct Used".to_owned(),
            ])
        );
        let nested = lazy
            .root(&db)
            .members(&db)
            .iter()
            .find_map(|member| match member {
                Member::Mod(mod_) if mod_.name(&db) == "nested" => Some(*mod_),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            member_labels(&db, nested),
            HashSet::from(["struct Nested".to_owned()])
        );
        assert!(
            rust.crates(&db)
                .iter()
                .all(|crate_| crate_.name(&db) != "unused_lazy")
        );

        let shim = rust.shim(&db).render(&db);
        assert!(shim.contains("lazy::Used"));
        assert!(!shim.contains("lazy::Unused"));
    }

    fn member_labels(db: &dyn Db, mod_: Mod<'_>) -> HashSet<String> {
        mod_.members(db)
            .iter()
            .map(|member| match member {
                Member::Struct(struct_) => format!("struct {}", struct_.name(db)),
                Member::Enum(enum_) => format!("enum {}", enum_.name(db)),
                Member::Function(function) => format!("fn {}", function.name(db)),
                Member::Mod(mod_) => format!("mod {}", mod_.name(db)),
            })
            .collect()
    }
}
