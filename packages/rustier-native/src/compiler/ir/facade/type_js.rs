use crate::compiler::{
    db::Db,
    ir::{
        facade::{
            FacadeClass, FacadeFieldPlain, FacadeFreeFunction, FacadeFunction, FacadeMethod,
            FacadeParam,
        },
        rust::{Type, TypeKind},
    },
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum FacadeTypeJs<'db> {
    Unit,
    boolean,
    string,
    number,
    unknown,
    Class(FacadeClass<'db>),
}

impl<'db> FacadeTypeJs<'db> {
    fn from_source(db: &'db dyn Db, source: Type<'db>) -> Self {
        if let Some(class) = source.facade_class(db) {
            return Self::Class(class);
        }

        match source.kind(db) {
            TypeKind::Unit => Self::Unit,
            TypeKind::bool => Self::boolean,
            TypeKind::char
            | TypeKind::i8
            | TypeKind::u8
            | TypeKind::i16
            | TypeKind::u16
            | TypeKind::i32
            | TypeKind::u32
            | TypeKind::f32
            | TypeKind::f64 => Self::number,
            TypeKind::Str | TypeKind::String => Self::string,
            TypeKind::JsValue => Self::unknown,
            TypeKind::Borrowed(borrowed) => Self::from_source(db, borrowed.inner(db)),
            TypeKind::Struct(_) | TypeKind::Enum(_) | TypeKind::Generic(_) => {
                unreachable!("unsupported Rust types do not enter the facade")
            }
        }
    }
}

impl<'db> FacadeFunction<'db> {
    pub fn ret_err_type_js(self, db: &'db dyn Db) -> Option<FacadeTypeJs<'db>> {
        self.ret_err_type(db)
            .map(|type_| FacadeTypeJs::from_source(db, type_))
    }

    pub fn ret_type_js(self, db: &'db dyn Db) -> FacadeTypeJs<'db> {
        match self {
            Self::Free(function) => function.ret_type_js(db),
            Self::Method(method) => method.ret_type_js(db),
        }
    }
}

impl<'db> FacadeFreeFunction<'db> {
    pub fn ret_type_js(self, db: &'db dyn Db) -> FacadeTypeJs<'db> {
        FacadeTypeJs::from_source(db, self.ret_type(db))
    }
}

impl<'db> FacadeMethod<'db> {
    pub fn ret_type_js(self, db: &'db dyn Db) -> FacadeTypeJs<'db> {
        FacadeTypeJs::from_source(db, self.ret_type(db))
    }
}

impl<'db> FacadeParam<'db> {
    pub fn type_js(self, db: &'db dyn Db) -> FacadeTypeJs<'db> {
        FacadeTypeJs::from_source(db, self.src_type(db))
    }
}

impl<'db> FacadeFieldPlain<'db> {
    pub fn type_js(self, db: &'db dyn Db) -> FacadeTypeJs<'db> {
        FacadeTypeJs::from_source(db, self.src_type(db))
    }
}
