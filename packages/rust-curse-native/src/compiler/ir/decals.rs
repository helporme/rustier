//! Intermediate representation used to generate TypeScript declarations.

pub mod ext;
mod lowering;

use std::collections::HashSet;

use crate::compiler::{
    FilePath,
    db::Db,
    ir::{
        rust::{Field, FieldName, Span, Variant},
        shim::{
            ShimClass, ShimClassSource, ShimFunctionEnumDiscriminator, ShimFunctionFree, ShimMod,
            ShimParam,
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

    pub fn span(self, db: &'db dyn Db) -> &'db Span {
        self.shim(db).src(db).span(db)
    }

    pub fn fns(self, db: &'db dyn Db) -> &'db Vec<DecalFunction<'db>> {
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

#[salsa::interned(debug)]
pub struct DecalFunction<'db> {
    shim: ShimFunctionFree<'db>,
}

impl<'db> DecalFunction<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.shim(db).src(db).name(db)
    }

    pub fn span(self, db: &'db dyn Db) -> &'db Span {
        self.shim(db).src(db).span(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.shim(db).src(db).docs(db)
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<DecalParam<'db>> {
        self.lower_params(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<DecalType<'db>> {
        self.lower_ret_type(db)
    }
}

#[salsa::interned(debug)]
pub struct DecalParam<'db> {
    shim: ShimParam<'db>,
}

impl<'db> DecalParam<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        let shim = self.shim(db);
        let function = match shim.parent(db) {
            crate::compiler::ir::shim::ShimFunction::Free(function) => function,
            _ => unreachable!("declaration parameters only belong to free functions"),
        };

        function.src(db).params(db)[shim.idx(db)].name(db)
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

    pub fn span(self, db: &'db dyn Db) -> &'db Span {
        match self.shim(db).src(db) {
            ShimClassSource::Struct(struct_) => struct_.span(db),
            ShimClassSource::Enum(enum_) => enum_.span(db),
        }
    }

    pub fn kind(self, db: &'db dyn Db) -> &'db DecalClassKind<'db> {
        self.lower_kind(db)
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

    pub fn span(self, db: &'db dyn Db) -> &'db Span {
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
        self.src(db).span(db).as_ref()
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).docs(db).as_ref()
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
