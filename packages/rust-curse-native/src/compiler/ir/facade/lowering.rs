use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::compiler::{
    db::Db,
    inputs::CompilationMetadata,
    ir::{
        facade::{
            FacadeClass, FacadeField, FacadeFieldDiscriminated, FacadeFieldDiscriminator,
            FacadeFieldPlain, FacadeFunction, FacadeImport, FacadeImportItem, FacadeMod,
            FacadeParam, FacadeType,
        },
        rust::FieldName,
        shim::{ShimClassSource, ShimFunction, ShimFunctionEnumDiscriminator},
    },
};

#[salsa::tracked]
impl<'db> FacadeImport<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_path(self, db: &'db dyn Db) -> String {
        let metadata = CompilationMetadata::get(db);
        let originating_from_path = self.originating_from(db).file_path(db);
        let to_path = match self.item(db) {
            FacadeImportItem::FfiFunction(_) | FacadeImportItem::FfiClass(_) => {
                &metadata.js_ffi_path
            }
            FacadeImportItem::FacadeClass(class) => class.parent(db).file_path(db),
        };

        compile_relative_path(originating_from_path, to_path).expect("relative path must exist")
    }
}

#[salsa::tracked]
impl<'db> FacadeMod<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_fns(self, db: &'db dyn Db) -> Vec<FacadeFunction<'db>> {
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
            .map(|item| FacadeImport::new(db, self, item))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_file_path(self, db: &'db dyn Db) -> PathBuf {
        let metadata = CompilationMetadata::get(db);
        let span = self.shim(db).src(db).span(db);
        metadata.js_mods_root_path.join(&span.src_path)
    }
}

#[salsa::tracked]
impl<'db> FacadeFunction<'db> {
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
            .ret_type(db)
            .map(|type_| type_.lower_facade(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_imports(self, db: &'db dyn Db) -> HashSet<FacadeImportItem<'db>> {
        let mut imports = HashSet::new();

        for param in self.params(db) {
            if let FacadeType::Class(class) = param.type_(db) {
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
    pub(super) fn lower_fields(self, db: &'db dyn Db) -> Vec<FacadeField<'db>> {
        let shim = self.shim(db);

        match shim.src(db) {
            ShimClassSource::Struct(_) => shim
                .fields(db)
                .iter()
                .map(|field| FacadeFieldPlain::new(db, field).as_field())
                .collect(),
            ShimClassSource::Enum(enum_) => {
                let mut groups: HashMap<&'db FieldName, HashMap<u32, FacadeFieldPlain<'db>>> =
                    HashMap::new();

                for (variant_idx, variant) in enum_.variants(db).iter().enumerate() {
                    for field in variant.fields(db) {
                        groups
                            .entry(field.name(db))
                            .or_default()
                            .entry(variant_idx as u32)
                            .or_insert_with(|| FacadeFieldPlain::new(db, field.shim(db)));
                    }
                }

                let mut fields = groups
                    .into_iter()
                    .map(|(name, sub)| {
                        FacadeFieldDiscriminated::new(db, self, name.to_owned(), sub).as_field()
                    })
                    .collect::<Vec<_>>();
                let variant_names = enum_
                    .variants(db)
                    .iter()
                    .map(|variant| variant.name(db).to_owned())
                    .collect();

                fields.push(
                    FacadeFieldDiscriminator::new(
                        db,
                        ShimFunctionEnumDiscriminator::new(db, enum_),
                        variant_names,
                    )
                    .as_field(),
                );

                fields
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

        for field in self.fields(db) {
            match field {
                FacadeField::Plain(field) => insert_plain_field_imports(db, *field, &mut imports),
                FacadeField::Discriminated(field) => {
                    for sub in field.sub(db).values() {
                        insert_plain_field_imports(db, *sub, &mut imports);
                    }
                }
                FacadeField::Discriminator(field) => {
                    imports.insert(FacadeImportItem::FfiFunction(field.ffi_getter(db)));
                }
            }
        }

        imports.insert(FacadeImportItem::FfiClass(self.ffi(db)));
        imports
    }
}

#[salsa::tracked]
impl<'db> FacadeFieldPlain<'db> {
    pub(super) fn lower_type(self, db: &'db dyn Db) -> FacadeType<'db> {
        self.shim(db).getter(db).ret_type(db).lower_facade(db)
    }
}

fn compile_relative_path(from_mod_path: &Path, to_mod_path: &Path) -> Option<String> {
    let mut from_components = from_mod_path.parent()?.components().peekable();
    let mut to_components = to_mod_path.components().peekable();

    while matches!(
        (from_components.peek(), to_components.peek()),
        (Some(from), Some(to)) if from == to
    ) {
        from_components.next();
        to_components.next();
    }

    if from_components.clone().any(|component| {
        matches!(
            component,
            std::path::Component::Prefix(_) | std::path::Component::RootDir
        )
    }) || to_components.clone().any(|component| {
        matches!(
            component,
            std::path::Component::Prefix(_) | std::path::Component::RootDir
        )
    }) {
        return None;
    }

    let mut path = String::new();
    let parent_count = from_components.count();
    if parent_count == 0 {
        path.push_str("./");
    } else {
        for _ in 0..parent_count {
            path.push_str("../");
        }
    }

    for (index, component) in to_components.enumerate() {
        if index > 0 {
            path.push('/');
        }
        path.push_str(&component.as_os_str().to_string_lossy());
    }

    Some(path)
}
