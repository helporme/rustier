//! Intermediate representation used to generate TypeScript declarations.

pub mod ext;
mod lowering;

use std::collections::HashSet;

use crate::compiler::{
    FilePath,
    db::Db,
    ir::{
        rust::{Field, FieldName, Function, Span, Variant},
        shim::{
            ShimClass, ShimClassSource, ShimFunction, ShimFunctionAssociated,
            ShimFunctionEnumDiscriminator, ShimFunctionFree, ShimMod, ShimParam,
        },
    },
};

#[salsa::tracked(debug)]
pub struct DecalImport<'db> {
    pub originating_from: DecalMod<'db>,
    pub class: DecalClass<'db>,
}

impl<'db> DecalImport<'db> {
    pub fn path(self, db: &'db dyn Db) -> &'db str {
        self.lower_path(db)
    }
}

#[salsa::interned(debug)]
pub struct DecalMod<'db> {
    shim: ShimMod<'db>,
}

impl<'db> DecalMod<'db> {
    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.shim(db).src(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.shim(db).src(db).span(db)
    }

    pub fn fns(self, db: &'db dyn Db) -> &'db Vec<DecalFreeFunction<'db>> {
        self.lower_fns(db)
    }

    pub fn classes(self, db: &'db dyn Db) -> &'db Vec<DecalClass<'db>> {
        self.lower_classes(db)
    }

    pub fn imports(self, db: &'db dyn Db) -> &'db HashSet<DecalImport<'db>> {
        self.lower_imports(db)
    }

    pub fn file_path(self, db: &'db dyn Db) -> &'db FilePath {
        self.lower_file_path(db)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum DecalFunction<'db> {
    Free(DecalFreeFunction<'db>),
    Method(DecalMethod<'db>),
}

impl<'db> DecalFunction<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        match self {
            Self::Free(function) => function.name(db),
            Self::Method(method) => method.name(db),
        }
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        match self {
            Self::Free(function) => function.span(db),
            Self::Method(method) => method.span(db),
        }
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        match self {
            Self::Free(function) => function.docs(db),
            Self::Method(method) => method.docs(db),
        }
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<DecalParam<'db>> {
        match self {
            Self::Free(function) => function.params(db),
            Self::Method(method) => method.params(db),
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<DecalType<'db>> {
        match self {
            Self::Free(function) => function.ret_type(db),
            Self::Method(method) => method.ret_type(db),
        }
    }
}

#[salsa::interned(debug)]
pub struct DecalFreeFunction<'db> {
    shim: ShimFunctionFree<'db>,
}

impl<'db> DecalFreeFunction<'db> {
    pub fn as_fn(self) -> DecalFunction<'db> {
        DecalFunction::Free(self)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).name(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).span(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).docs(db)
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<DecalParam<'db>> {
        self.lower_params(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<DecalType<'db>> {
        self.lower_ret_type(db)
    }

    fn src(self, db: &'db dyn Db) -> Function<'db> {
        self.shim(db).src(db)
    }
}

#[salsa::interned(debug)]
pub struct DecalMethod<'db> {
    shim: ShimFunctionAssociated<'db>,
}

impl<'db> DecalMethod<'db> {
    pub fn as_fn(self) -> DecalFunction<'db> {
        DecalFunction::Method(self)
    }

    pub fn is_static(self, db: &'db dyn Db) -> bool {
        self.src(db).receiver(db).is_none()
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).name(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).span(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).docs(db)
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<DecalParam<'db>> {
        self.lower_params(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<DecalType<'db>> {
        self.lower_ret_type(db)
    }

    fn src(self, db: &'db dyn Db) -> Function<'db> {
        self.shim(db).src(db)
    }
}

#[salsa::interned(debug)]
pub struct DecalParam<'db> {
    shim: ShimParam<'db>,
}

impl<'db> DecalParam<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        let shim = self.shim(db);
        let (function, param_idx) = match shim.parent(db) {
            ShimFunction::Free(function) => (function.src(db), shim.idx(db)),
            ShimFunction::Associated(function) => {
                let receiver_offset = usize::from(function.src(db).receiver(db).is_some());
                (function.src(db), shim.idx(db) - receiver_offset)
            }
            _ => unreachable!("declaration parameters only belong to source-defined functions"),
        };

        function.params(db)[param_idx].name(db)
    }

    pub fn type_(self, db: &'db dyn Db) -> DecalType<'db> {
        self.lower_type(db)
    }
}

#[salsa::interned(debug)]
pub struct DecalClass<'db> {
    shim: ShimClass<'db>,
}

impl<'db> DecalClass<'db> {
    pub fn parent(self, db: &'db dyn Db) -> DecalMod<'db> {
        self.shim(db).parent(db).decals(db)
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self.shim(db).src(db) {
            ShimClassSource::Struct(struct_) => struct_.name(db),
            ShimClassSource::Enum(enum_) => enum_.name(db),
        }
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        match self.shim(db).src(db) {
            ShimClassSource::Struct(struct_) => struct_.docs(db),
            ShimClassSource::Enum(enum_) => enum_.docs(db),
        }
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        match self.shim(db).src(db) {
            ShimClassSource::Struct(struct_) => struct_.span(db),
            ShimClassSource::Enum(enum_) => enum_.span(db),
        }
    }

    pub fn kind(self, db: &'db dyn Db) -> &'db DecalClassKind<'db> {
        self.lower_kind(db)
    }

    pub fn methods(self, db: &'db dyn Db) -> &'db Vec<DecalMethod<'db>> {
        self.lower_methods(db)
    }

    pub fn constructor(self, db: &'db dyn Db) -> Option<DecalMethod<'db>> {
        self.lower_constructor(db)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, salsa::Update)]
pub enum DecalClassKind<'db> {
    Plain {
        fields: Vec<DecalField<'db>>,
    },
    DiscriminatedUnion {
        variants: Vec<DecalVariant<'db>>,
        discriminator: DecalDiscriminator<'db>,
    },
}

#[salsa::tracked(debug)]
pub struct DecalVariant<'db> {
    src: Variant<'db>,
    #[returns(ref)]
    #[tracked]
    pub name: String,
}

impl<'db> DecalVariant<'db> {
    pub fn source_name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).name(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).span(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).docs(db)
    }

    pub fn fields(self, db: &'db dyn Db) -> &'db Vec<DecalField<'db>> {
        self.lower_fields(db)
    }
}

#[salsa::interned(debug)]
pub struct DecalField<'db> {
    src: Field<'db>,
}

impl<'db> DecalField<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db FieldName {
        self.src(db).name(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).span(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).docs(db)
    }

    pub fn is_readonly(self, db: &'db dyn Db) -> bool {
        self.src(db).is_readonly(db)
    }

    pub fn type_(self, db: &'db dyn Db) -> DecalType<'db> {
        self.lower_type(db)
    }
}

#[salsa::interned(debug)]
pub struct DecalDiscriminator<'db> {
    src: ShimFunctionEnumDiscriminator<'db>,
}

impl<'db> DecalDiscriminator<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        self.src(db).facade(db).name(db)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum DecalType<'db> {
    boolean,
    string,
    number,
    Class(DecalClass<'db>),
}
