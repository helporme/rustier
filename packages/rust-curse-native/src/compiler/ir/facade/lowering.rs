use std::collections::{BTreeMap, HashMap, HashSet};

use crate::compiler::{
    db::Db,
    inputs::FfiLayout,
    ir::{
        facade::{
            FacadeClass, FacadeClassKind, FacadeFieldDiscriminated, FacadeFieldDiscriminator,
            FacadeFieldPlain, FacadeFreeFunction, FacadeImport, FacadeImportItem, FacadeMethod,
            FacadeMod, FacadeParam, FacadeType,
        },
        rust::FieldName,
        shim::{ShimClassSource, ShimFunction, ShimFunctionEnumDiscriminator, ShimType},
    },
    mangle::to_camel_case,
    path::FilePath,
};

#[salsa::tracked]
impl<'db> FacadeImport<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_path(self, db: &'db dyn Db) -> String {
        let layout = FfiLayout::get(db);
        let originating_from_path = self.originating_from(db).file_path(db);
        let to_path = match self.item(db) {
            FacadeImportItem::FfiFunction(_) | FacadeImportItem::FfiClass(_) => &layout.ffi_js_file,
            FacadeImportItem::FacadeClass(class) => class.parent(db).file_path(db),
        };

        originating_from_path
            .relative_path(to_path)
            .expect("relative path must exist")
    }
}

#[salsa::tracked]
impl<'db> FacadeMod<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_fns(self, db: &'db dyn Db) -> Vec<FacadeFreeFunction<'db>> {
        self.shim(db)
            .fns(db)
            .iter()
            .filter_map(|function| match function {
                ShimFunction::Free(free) => Some(free.facade(db)),
                _ => None,
            })
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_classes(self, db: &'db dyn Db) -> Vec<FacadeClass<'db>> {
        self.shim(db)
            .classes(db)
            .iter()
            .map(|class| (*class).facade(db))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_imports(self, db: &'db dyn Db) -> HashSet<FacadeImport<'db>> {
        let mut items = HashSet::new();

        for function in self.fns(db) {
            items.extend(function.imports(db).iter().copied());
        }

        for class in self.classes(db) {
            items.extend(class.imports(db).iter().copied());
        }

        items
            .into_iter()
            .filter(|item| {
                !matches!(
                    item,
                    FacadeImportItem::FacadeClass(class) if class.parent(db) == self
                )
            })
            .map(|item| FacadeImport::new(db, self, item))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_file_path(self, db: &'db dyn Db) -> FilePath {
        self.shim(db).src(db).file_path(db).to_owned()
    }
}

#[salsa::tracked]
impl<'db> FacadeFreeFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_params(self, db: &'db dyn Db) -> Vec<FacadeParam<'db>> {
        self.shim(db)
            .params(db)
            .iter()
            .copied()
            .map(|param| FacadeParam::new(db, param))
            .collect()
    }

    pub(super) fn lower_ret_type(self, db: &'db dyn Db) -> Option<FacadeType<'db>> {
        self.shim(db)
            .as_fn()
            .ret_type(db)
            .map(|type_| type_.lower_facade(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_imports(self, db: &'db dyn Db) -> HashSet<FacadeImportItem<'db>> {
        let mut imports = HashSet::from([FacadeImportItem::FfiFunction(self.ffi(db))]);

        for type_ in self
            .params(db)
            .iter()
            .map(|param| param.type_(db))
            .chain(self.ret_type(db))
        {
            if let FacadeType::Class(class) = type_ {
                imports.insert(FacadeImportItem::FacadeClass(class));
            }
        }

        imports
    }
}

#[salsa::tracked]
impl<'db> FacadeMethod<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_params(self, db: &'db dyn Db) -> Vec<FacadeParam<'db>> {
        self.shim(db)
            .params_without_receiver(db)
            .iter()
            .copied()
            .map(|param| FacadeParam::new(db, param))
            .collect()
    }

    pub(super) fn lower_ret_type(self, db: &'db dyn Db) -> Option<FacadeType<'db>> {
        self.shim(db)
            .ret_type(db)
            .map(|type_| type_.lower_facade(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_imports(self, db: &'db dyn Db) -> HashSet<FacadeImportItem<'db>> {
        let mut imports = HashSet::from([FacadeImportItem::FfiFunction(self.ffi(db))]);
        let owner = self.class(db);

        for type_ in self
            .params(db)
            .iter()
            .map(|param| param.type_(db))
            .chain(self.ret_type(db))
        {
            if let FacadeType::Class(class) = type_
                && class != owner
            {
                imports.insert(FacadeImportItem::FacadeClass(class));
            }
        }

        imports
    }
}

#[salsa::tracked]
impl<'db> FacadeParam<'db> {
    pub(super) fn lower_type(self, db: &'db dyn Db) -> FacadeType<'db> {
        self.shim(db).type_(db).lower_facade(db)
    }
}

#[salsa::tracked]
impl<'db> FacadeClass<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_methods(self, db: &'db dyn Db) -> Vec<FacadeMethod<'db>> {
        self.shim(db)
            .associated_fns(db)
            .iter()
            .map(|function| function.facade(db))
            .collect()
    }

    #[salsa::tracked]
    pub(super) fn lower_constructor(self, db: &'db dyn Db) -> Option<FacadeMethod<'db>> {
        let ShimClassSource::Struct(_) = self.shim(db).src(db) else {
            return None;
        };

        self.methods(db).iter().copied().find(|method| {
            method.name(db) == "new"
                && method.is_static(db)
                && matches!(
                    method.shim(db).ret_type(db),
                    Some(ShimType::Class {
                        is_borrowed: false,
                        inner,
                    }) if inner == self.shim(db)
                )
        })
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_kind(self, db: &'db dyn Db) -> FacadeClassKind<'db> {
        let shim = self.shim(db);

        match shim.src(db) {
            ShimClassSource::Struct(_) => FacadeClassKind::Plain {
                fields: shim
                    .fields(db)
                    .iter()
                    .map(|field| FacadeFieldPlain::new(db, field))
                    .collect(),
            },
            ShimClassSource::Enum(enum_) => {
                let mut groups: Vec<(FieldName, BTreeMap<u32, FacadeFieldPlain<'db>>)> = vec![];
                let exported_fields = shim
                    .fields(db)
                    .iter()
                    .map(|field| field.src(db))
                    .collect::<HashSet<_>>();

                for (variant_idx, variant) in enum_.variants(db).iter().enumerate() {
                    for field in variant
                        .fields(db)
                        .iter()
                        .copied()
                        .filter(|field| exported_fields.contains(field))
                    {
                        let name = field.name(db);
                        let sub = if let Some((_, sub)) =
                            groups.iter_mut().find(|(group_name, _)| group_name == name)
                        {
                            sub
                        } else {
                            groups.push((name.clone(), BTreeMap::new()));
                            &mut groups.last_mut().expect("field group was just inserted").1
                        };

                        sub.entry(variant_idx as u32)
                            .or_insert_with(|| FacadeFieldPlain::new(db, field.shim(db)));
                    }
                }

                let fields = groups
                    .into_iter()
                    .map(|(name, sub)| FacadeFieldDiscriminated::new(db, self, name, sub))
                    .collect::<Vec<_>>();

                FacadeClassKind::DiscriminatedUnion {
                    fields,
                    discriminator: FacadeFieldDiscriminator::new(
                        db,
                        ShimFunctionEnumDiscriminator::new(db, enum_),
                    ),
                }
            }
        }
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_imports(self, db: &'db dyn Db) -> HashSet<FacadeImportItem<'db>> {
        let mut imports = HashSet::new();

        fn insert_plain_field_imports<'db>(
            db: &'db dyn Db,
            field: FacadeFieldPlain<'db>,
            imports: &mut HashSet<FacadeImportItem<'db>>,
        ) {
            imports.insert(FacadeImportItem::FfiFunction(field.ffi_getter(db)));
            if let Some(setter) = field.ffi_setter(db) {
                imports.insert(FacadeImportItem::FfiFunction(setter));
            }
            if let FacadeType::Class(class) = field.type_(db) {
                imports.insert(FacadeImportItem::FacadeClass(class));
            }
        }

        match self.kind(db) {
            FacadeClassKind::Plain { fields } => {
                for field in fields {
                    insert_plain_field_imports(db, *field, &mut imports);
                }
            }
            FacadeClassKind::DiscriminatedUnion {
                fields,
                discriminator,
            } => {
                for field in fields {
                    for sub in field.sub(db).values() {
                        insert_plain_field_imports(db, *sub, &mut imports);
                    }
                }

                imports.insert(FacadeImportItem::FfiFunction(discriminator.ffi_getter(db)));
            }
        }

        for method in self.methods(db) {
            imports.extend(method.imports(db).iter().copied());
        }

        imports.insert(FacadeImportItem::FfiClass(self.ffi(db)));
        imports
    }
}

#[salsa::tracked]
impl<'db> FacadeFieldDiscriminator<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_name(self, db: &'db dyn Db) -> String {
        let enum_ = self.shim(db).src(db);

        if enum_.variants(db).iter().any(|variant| {
            variant
                .fields(db)
                .iter()
                .any(|field| matches!(field.name(db), FieldName::Name(name) if name == "type"))
        }) {
            "$type".to_owned()
        } else {
            "type".to_owned()
        }
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_variant_names(self, db: &'db dyn Db) -> Vec<String> {
        let enum_ = self.shim(db).src(db);
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
}

#[salsa::tracked]
impl<'db> FacadeFieldPlain<'db> {
    pub(super) fn lower_type(self, db: &'db dyn Db) -> FacadeType<'db> {
        self.shim(db).getter(db).ret_type(db).lower_facade(db)
    }
}

impl<'db> ShimType<'db> {
    pub(super) fn lower_facade(self, db: &'db dyn Db) -> FacadeType<'db> {
        match self {
            ShimType::f64 => FacadeType::number,
            ShimType::String => FacadeType::string,
            ShimType::bool => FacadeType::boolean,
            ShimType::Class { inner, .. } => FacadeType::Class(inner.facade(db)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::{
        db::RootDatabase,
        ir::{
            facade::{FacadeClass, FacadeClassKind},
            rust::Rust,
        },
        test::{build_test_db, format_rust_source},
    };

    #[test]
    fn enum_variant_tags_use_lower_camel_case_and_preserve_collisions() {
        let db = build_db(stringify! {
            pub enum Test {
                FirstVariant,
                HTTPRequest,
                HttpRequest,
            }
        });
        let class = only_facade_class(&db);
        let FacadeClassKind::DiscriminatedUnion { discriminator, .. } = class.kind(&db) else {
            panic!("enum should lower to a discriminated union");
        };

        assert_eq!(
            discriminator.variant_names(&db),
            &["firstVariant", "HTTPRequest", "HttpRequest"]
        );
        assert_eq!(discriminator.name(&db), "type");
    }

    #[test]
    fn enum_discriminator_avoids_a_real_type_field() {
        let db = build_db(stringify! {
            pub enum Test {
                Variant {
                    r#type: u32,
                },
            }
        });
        let class = only_facade_class(&db);
        let FacadeClassKind::DiscriminatedUnion { discriminator, .. } = class.kind(&db) else {
            panic!("enum should lower to a discriminated union");
        };

        assert_eq!(discriminator.name(&db), "$type");
    }

    fn build_db(source: &str) -> RootDatabase {
        build_test_db(format_rust_source(source), &[])
    }

    fn only_facade_class(db: &RootDatabase) -> FacadeClass<'_> {
        let root = Rust::get(db)
            .crates(db)
            .first()
            .expect("test crate should be lowered")
            .root(db)
            .shim(db)
            .facade(db);
        let [class] = root.classes(db).as_slice() else {
            panic!("test crate root should contain exactly one facade class");
        };

        *class
    }
}
