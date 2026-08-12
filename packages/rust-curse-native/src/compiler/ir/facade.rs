use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::compiler::{
    db::Db,
    inputs::CompilationMetadata,
    ir::{
        ffi::{FfiClass, FfiFunction, FfiType},
        rust::FieldName,
        shim::{
            ShimClass, ShimClassSource, ShimField, ShimFunction, ShimFunctionEnumDiscriminator,
            ShimFunctionFree, ShimMod, ShimParam, ShimValueTypeKind,
        },
    },
};

#[salsa::tracked(debug)]
pub struct FacadeImport<'db> {
    pub originating_from: FacadeMod<'db>,
    pub item: FacadeImportItem<'db>,
}

#[salsa::tracked]
impl<'db> FacadeImport<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn path(self, db: &dyn Db) -> String {
        let metadata = CompilationMetadata::get(db);

        let originating_from_path = self.originating_from(db).file_path(db);
        let to_path = match self.item(db) {
            FacadeImportItem::FfiFunction(_) | FacadeImportItem::FfiClass(_) => {
                &metadata.js_ffi_path
            }
            FacadeImportItem::FacadeClass(cls) => cls.parent(db).file_path(db),
        };

        Self::compile_relative_path(&originating_from_path, &to_path)
            .expect("relative path must exist")
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
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeImportItem<'db> {
    FfiFunction(FfiFunction<'db>),
    FfiClass(FfiClass<'db>),
    FacadeClass(FacadeClass<'db>),
}

impl<'db> FacadeImportItem<'db> {
    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        match self {
            FacadeImportItem::FfiFunction(f) => f.symbol(db),
            FacadeImportItem::FfiClass(c) => c.symbol(db),
            FacadeImportItem::FacadeClass(c) => c.ffi(db).symbol(db),
        }
    }
}

#[salsa::interned(debug)]
pub struct FacadeMod<'db> {
    shim: ShimMod<'db>,
}

#[salsa::tracked]
impl<'db> FacadeMod<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn fns(self, db: &'db dyn Db) -> Vec<FacadeFunction<'db>> {
        self.shim(db)
            .fns(db)
            .iter()
            .filter_map(|fun| match fun {
                ShimFunction::Free(free) => Some(free.facade(db)),
                _ => None,
            })
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub fn classes(self, db: &'db dyn Db) -> Vec<FacadeClass<'db>> {
        self.shim(db)
            .classes(db)
            .iter()
            .map(|c| c.facade(db))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub fn imports(self, db: &'db dyn Db) -> HashSet<FacadeImport<'db>> {
        let mut items = HashSet::new();

        for fun in self.fns(db) {
            items.extend(fun.imports(db).iter().copied());
        }

        for cls in self.classes(db) {
            items.extend(cls.imports(db).iter().copied());
        }

        items
            .into_iter()
            .map(|i| FacadeImport::new(db, self, i))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub fn file_path(self, db: &'db dyn Db) -> PathBuf {
        let metadata = CompilationMetadata::get(db);

        let span = self.shim(db).src(db).span(db);
        metadata.js_mods_root_path.join(&span.src_path)
    }
}

#[salsa::interned(debug)]
pub struct FacadeFunction<'db> {
    shim: ShimFunctionFree<'db>,
}

#[salsa::tracked]
impl<'db> FacadeFunction<'db> {
    pub fn name(self, db: &'db dyn Db) -> &String {
        self.shim(db).src(db).name(db)
    }

    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).ffi(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, _db: &'db dyn Db) -> Vec<FacadeParam<'db>> {
        todo!()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<FacadeType<'db>> {
        todo!()
    }

    #[salsa::tracked(returns(ref))]
    fn imports(self, db: &'db dyn Db) -> HashSet<FacadeImportItem<'db>> {
        let mut imports = HashSet::new();
        for param in self.params(db) {
            if let FacadeTypeKind::Class(class) = param.type_(db).kind {
                imports.insert(FacadeImportItem::FacadeClass(class));
            }
        }
        imports
    }
}

#[salsa::interned(debug)]
pub struct FacadeParam<'db> {
    shim: ShimParam<'db>,
}

#[salsa::tracked]
impl<'db> FacadeParam<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn type_(self, _db: &'db dyn Db) -> FacadeType<'db> {
        todo!()
    }
}

#[salsa::interned(debug)]
pub struct FacadeClass<'db> {
    shim: ShimClass<'db>,
}

#[salsa::tracked]
impl<'db> FacadeClass<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        self.shim(db).ffi(db)
    }

    pub fn parent(self, db: &'db dyn Db) -> FacadeMod<'db> {
        self.shim(db).parent(db).facade(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self.shim(db).src(db) {
            ShimClassSource::Struct(s) => s.name(db),
            ShimClassSource::Enum(e) => e.name(db),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<FacadeField<'db>> {
        let shim = self.shim(db);

        match shim.src(db) {
            ShimClassSource::Struct(..) => shim
                .fields(db)
                .iter()
                .map(|field| FacadeFieldPlain::new(db, field).as_field())
                .collect(),

            ShimClassSource::Enum(enum_) => {
                let mut groups: HashMap<&'db FieldName, HashMap<u32, FacadeFieldPlain<'db>>> =
                    HashMap::new();

                for (variant_idx, variant) in enum_.variants(db).iter().enumerate() {
                    for field in variant.fields(db) {
                        let name = field.name(db);
                        let group = groups.entry(name).or_default();
                        group
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
                    .map(|v| v.name(db).to_owned())
                    .collect::<Vec<_>>();

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
    fn imports(self, db: &'db dyn Db) -> HashSet<FacadeImportItem<'db>> {
        let mut imports = HashSet::new();

        fn insert_plain_field_imports<'db>(
            db: &'db dyn Db,
            field: &FacadeFieldPlain<'db>,
            imports: &mut HashSet<FacadeImportItem<'db>>,
        ) {
            imports.insert(FacadeImportItem::FfiFunction(field.ffi_getter(db)));
            if let Some(setter) = field.ffi_setter(db) {
                imports.insert(FacadeImportItem::FfiFunction(setter));
            }
            if let FacadeTypeKind::Class(class) = field.type_(db).kind {
                imports.insert(FacadeImportItem::FacadeClass(class));
            }
        }

        for field in self.fields(db) {
            match field {
                FacadeField::Plain(f) => insert_plain_field_imports(db, f, &mut imports),
                FacadeField::Discriminated(f) => {
                    for sub in f.sub(db).values() {
                        insert_plain_field_imports(db, sub, &mut imports);
                    }
                }
                FacadeField::Discriminator(f) => {
                    imports.insert(FacadeImportItem::FfiFunction(f.ffi(db)));
                }
            }
        }

        imports.insert(FacadeImportItem::FfiClass(self.ffi(db)));

        imports
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, salsa::Update, salsa::Supertype)]
pub enum FacadeField<'db> {
    Plain(FacadeFieldPlain<'db>),
    Discriminated(FacadeFieldDiscriminated<'db>),
    Discriminator(FacadeFieldDiscriminator<'db>),
}

#[salsa::interned(debug)]
pub struct FacadeFieldPlain<'db> {
    shim: ShimField<'db>,
}

impl<'db> FacadeFieldPlain<'db> {
    pub fn as_field(self) -> FacadeField<'db> {
        FacadeField::Plain(self)
    }

    pub fn ffi_setter(self, db: &'db dyn Db) -> Option<FfiFunction<'db>> {
        self.shim(db).setter(db).map(|f| f.ffi(db))
    }

    pub fn ffi_getter(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).getter(db).ffi(db)
    }

    pub fn type_(self, db: &'db dyn Db) -> FacadeType<'db> {
        let shim_type = self.shim(db).getter(db).ret_type(db);

        match shim_type.kind {
            ShimValueTypeKind::bool => FacadeType {
                ffi: shim_type.ffi(db),
                kind: FacadeTypeKind::Boolean,
            },
            ShimValueTypeKind::i8
            | ShimValueTypeKind::u8
            | ShimValueTypeKind::i16
            | ShimValueTypeKind::u16
            | ShimValueTypeKind::i32
            | ShimValueTypeKind::u32
            | ShimValueTypeKind::f64 => FacadeType {
                ffi: shim_type.ffi(db),
                kind: FacadeTypeKind::Number,
            },
            ShimValueTypeKind::String => FacadeType {
                ffi: shim_type.ffi(db),
                kind: FacadeTypeKind::String,
            },
            ShimValueTypeKind::Class(class) => FacadeType {
                ffi: shim_type.ffi(db),
                kind: FacadeTypeKind::Class(class.facade(db)),
            },
            ShimValueTypeKind::JsAny => unreachable!(),
        }
    }
}

#[salsa::tracked(debug)]
pub struct FacadeFieldDiscriminated<'db> {
    parent: FacadeClass<'db>,
    #[returns(ref)]
    pub name: FieldName,
    #[returns(ref)]
    #[tracked]
    pub sub: HashMap<u32, FacadeFieldPlain<'db>>,
}

#[salsa::tracked]
impl<'db> FacadeFieldDiscriminated<'db> {
    pub fn as_field(self) -> FacadeField<'db> {
        FacadeField::Discriminated(self)
    }
}

#[salsa::tracked(debug)]
pub struct FacadeFieldDiscriminator<'db> {
    shim: ShimFunctionEnumDiscriminator<'db>,
    #[returns(ref)]
    #[tracked]
    pub variant_names: Vec<String>,
}

#[salsa::tracked]
impl<'db> FacadeFieldDiscriminator<'db> {
    pub fn as_field(self) -> FacadeField<'db> {
        FacadeField::Discriminator(self)
    }

    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        self.shim(db).ffi(db)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, salsa::Update)]
pub enum FacadeTypeKind<'db> {
    Number,
    String,
    Boolean,
    Class(FacadeClass<'db>),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, salsa::Update)]
pub struct FacadeType<'db> {
    pub ffi: FfiType<'db>,
    pub kind: FacadeTypeKind<'db>,
}
