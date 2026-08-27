use std::collections::{HashMap, HashSet};

use crate::compiler::{
    FilePath,
    db::Db,
    ir::{
        decals::{
            DecalClass, DecalClassKind, DecalDiscriminator, DecalField, DecalFunction,
            DecalFunctionKind, DecalImport, DecalMod, DecalParam, DecalType, DecalVariant,
        },
        shim::{ShimClassSource, ShimFunction, ShimFunctionEnumDiscriminator, ShimType},
    },
    mangle::to_camel_case,
};

#[salsa::tracked]
impl<'db> DecalImport<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_path(self, db: &'db dyn Db) -> String {
        self.originating_from(db)
            .file_path(db)
            .relative_path(self.class(db).parent(db).file_path(db))
            .expect("relative path must exist")
    }
}

#[salsa::tracked]
impl<'db> DecalMod<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_fns(self, db: &'db dyn Db) -> Vec<DecalFunction<'db>> {
        self.shim(db)
            .fns(db)
            .iter()
            .filter_map(|function| match function {
                ShimFunction::Free(function) => Some(function.decals(db)),
                _ => None,
            })
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_classes(self, db: &'db dyn Db) -> Vec<DecalClass<'db>> {
        self.shim(db)
            .classes(db)
            .iter()
            .map(|class| class.decals(db))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_imports(self, db: &'db dyn Db) -> HashSet<DecalImport<'db>> {
        let mut classes = HashSet::new();
        let mut insert_type = |type_: DecalType<'db>| {
            if let DecalType::Class(class) = type_
                && class.parent(db) != self
            {
                classes.insert(class);
            }
        };

        for function in self.fns(db) {
            for param in function.params(db) {
                insert_type(param.type_(db));
            }
            if let Some(type_) = function.ret_type(db) {
                insert_type(type_);
            }
        }

        for class in self.classes(db) {
            match class.kind(db) {
                DecalClassKind::Plain { fields } => {
                    for field in fields {
                        insert_type(field.type_(db));
                    }
                }
                DecalClassKind::DiscriminatedUnion { variants, .. } => {
                    for variant in variants {
                        for field in variant.fields(db) {
                            insert_type(field.type_(db));
                        }
                    }
                }
            }

            for method in class.methods(db) {
                for param in method.params(db) {
                    insert_type(param.type_(db));
                }
                if let Some(type_) = method.ret_type(db) {
                    insert_type(type_);
                }
            }
        }

        classes
            .into_iter()
            .map(|class| DecalImport::new(db, self, class))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_file_path(self, db: &'db dyn Db) -> FilePath {
        self.shim(db).src(db).file_path(db).to_owned()
    }
}

#[salsa::tracked]
impl<'db> DecalFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_params(self, db: &'db dyn Db) -> Vec<DecalParam<'db>> {
        let params = match self.shim(db) {
            ShimFunction::Free(function) => function.params(db).as_slice(),
            ShimFunction::Associated(function) => function.params_without_receiver(db),
            _ => unreachable!("declaration functions must be source-defined functions"),
        };

        params
            .iter()
            .map(|param| DecalParam::new(db, *param))
            .collect()
    }

    pub(super) fn lower_ret_type(self, db: &'db dyn Db) -> Option<DecalType<'db>> {
        self.shim(db)
            .ret_type(db)
            .map(|type_| type_.lower_decals(db))
    }
}

#[salsa::tracked]
impl<'db> DecalParam<'db> {
    pub(super) fn lower_type(self, db: &'db dyn Db) -> DecalType<'db> {
        self.shim(db).type_(db).lower_decals(db)
    }
}

#[salsa::tracked]
impl<'db> DecalClass<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_methods(self, db: &'db dyn Db) -> Vec<DecalFunction<'db>> {
        self.shim(db)
            .associated_fns(db)
            .iter()
            .map(|function| {
                let function = function.decals(db);
                debug_assert!(matches!(
                    function.kind(db),
                    DecalFunctionKind::Method | DecalFunctionKind::StaticMethod,
                ));
                function
            })
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_kind(self, db: &'db dyn Db) -> DecalClassKind<'db> {
        match self.shim(db).src(db) {
            ShimClassSource::Struct(_) => DecalClassKind::Plain {
                fields: self
                    .shim(db)
                    .fields(db)
                    .iter()
                    .map(|field| DecalField::new(db, field.src(db)))
                    .collect(),
            },
            ShimClassSource::Enum(enum_) => {
                let variants = enum_
                    .variants(db)
                    .iter()
                    .zip(lower_variant_names(db, enum_))
                    .map(|(variant, name)| DecalVariant::new(db, *variant, name))
                    .collect();

                DecalClassKind::DiscriminatedUnion {
                    variants,
                    discriminator: DecalDiscriminator::new(
                        db,
                        ShimFunctionEnumDiscriminator::new(db, enum_),
                    ),
                }
            }
        }
    }
}

#[salsa::tracked]
impl<'db> DecalVariant<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_fields(self, db: &'db dyn Db) -> Vec<DecalField<'db>> {
        let class = self.src(db).parent(db).shim(db);

        self.src(db)
            .fields(db)
            .iter()
            .filter(|field| {
                class
                    .fields(db)
                    .iter()
                    .any(|shim_field| shim_field.src(db) == **field)
            })
            .map(|field| DecalField::new(db, *field))
            .collect()
    }
}

#[salsa::tracked]
impl<'db> DecalField<'db> {
    pub(super) fn lower_type(self, db: &'db dyn Db) -> DecalType<'db> {
        self.src(db)
            .shim(db)
            .getter(db)
            .ret_type(db)
            .lower_decals(db)
    }
}

impl<'db> ShimType<'db> {
    pub(super) fn lower_decals(self, db: &'db dyn Db) -> DecalType<'db> {
        match self {
            ShimType::f64 => DecalType::number,
            ShimType::String => DecalType::string,
            ShimType::bool => DecalType::boolean,
            ShimType::Class { inner, .. } => DecalType::Class(inner.decals(db)),
        }
    }
}

fn lower_variant_names<'db>(
    db: &'db dyn Db,
    enum_: crate::compiler::ir::rust::Enum<'db>,
) -> Vec<String> {
    let camel_case_names = enum_
        .variants(db)
        .iter()
        .map(|variant| to_camel_case(variant.name(db)))
        .collect::<Vec<_>>();
    let mut occurrences = HashMap::new();

    for name in &camel_case_names {
        *occurrences.entry(name.clone()).or_insert(0) += 1;
    }

    enum_
        .variants(db)
        .iter()
        .zip(camel_case_names)
        .map(|(variant, camel_case_name)| {
            if occurrences[&camel_case_name] > 1 {
                variant.name(db).to_owned()
            } else {
                camel_case_name
            }
        })
        .collect()
}
