use itertools::Either;

use crate::compiler::{
    db::Db,
    ir::facade::{
        FacadeClass, FacadeClassDiscriminated, FacadeClassPlain, FacadeFieldPlain,
        FacadeFreeFunction, FacadeFunction, FacadeGeneric, FacadeMethod, FacadeParam,
        type_js::FacadeTypeJs,
    },
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum FacadeTypeTs<'db> {
    boolean,
    string,
    number,
    Class {
        inner: FacadeClass<'db>,
        coerced: bool,
    },
}

impl<'db> FacadeTypeTs<'db> {
    pub fn try_as_coerced(self, db: &'db dyn Db) -> Self {
        match self {
            Self::Class { inner, .. } => Self::Class {
                inner,
                coerced: inner.coerce(db).is_some(),
            },
            type_ => type_,
        }
    }

    pub fn decl_imports(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeClass<'db>> + 'db {
        match self {
            Self::Class { inner, .. } => Either::Left(
                std::iter::once(inner).chain(inner.type_name(db).decl_imports(db).iter().copied()),
            ),
            _ => Either::Right(std::iter::empty()),
        }
    }
}

impl<'db> From<FacadeTypeJs<'db>> for FacadeTypeTs<'db> {
    fn from(value: FacadeTypeJs<'db>) -> Self {
        match value {
            FacadeTypeJs::boolean => Self::boolean,
            FacadeTypeJs::string => Self::string,
            FacadeTypeJs::number => Self::number,
            FacadeTypeJs::Class(inner) => Self::Class {
                inner,
                coerced: false,
            },
        }
    }
}

#[salsa::interned(debug)]
pub struct FacadeTypeNameTs<'db> {
    #[returns(ref)]
    pub name: String,
    #[returns(ref)]
    pub args: Option<Vec<FacadeGeneric<'db>>>,
}

#[salsa::tracked]
impl<'db> FacadeTypeNameTs<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn decl_imports(self, db: &'db dyn Db) -> Vec<FacadeClass<'db>> {
        self.args(db)
            .iter()
            .flatten()
            .flat_map(move |generic| match generic {
                FacadeGeneric::Class(class) => Either::Left(
                    std::iter::once(*class)
                        .chain(class.type_name(db).decl_imports(db).iter().copied()),
                ),
                FacadeGeneric::Symbol(_) => Either::Right(std::iter::empty()),
            })
            .collect()
    }
}

impl<'db> FacadeClass<'db> {
    pub fn is_template_instance(self, db: &'db dyn Db) -> bool {
        match self {
            Self::Plain(class) => class.is_template_instance(db),
            Self::Discriminated(class) => class.is_template_instance(db),
        }
    }

    pub fn type_name(self, db: &'db dyn Db) -> FacadeTypeNameTs<'db> {
        let segment = match self {
            Self::Plain(class) => class.src(db).segment(db),
            Self::Discriminated(class) => class.src(db).segment(db),
        };
        let args = self
            .is_template_instance(db)
            .then(|| self.generics(db).collect());

        FacadeTypeNameTs::new(db, segment.name(db).clone(), args)
    }
}

impl<'db> FacadeClassPlain<'db> {
    pub fn is_template_instance(self, db: &'db dyn Db) -> bool {
        !self.src(db).gen_args(db).is_empty()
    }
}

impl<'db> FacadeClassDiscriminated<'db> {
    pub fn is_template_instance(self, db: &'db dyn Db) -> bool {
        !self.src(db).gen_args(db).is_empty()
    }
}

impl<'db> FacadeFunction<'db> {
    pub fn ret_type_ts(self, db: &'db dyn Db) -> Option<FacadeTypeTs<'db>> {
        self.ret_type_js(db).map(Into::into)
    }
}

impl<'db> FacadeFreeFunction<'db> {
    pub fn ret_type_ts(self, db: &'db dyn Db) -> Option<FacadeTypeTs<'db>> {
        self.ret_type_js(db).map(Into::into)
    }
}

impl<'db> FacadeMethod<'db> {
    pub fn ret_type_ts(self, db: &'db dyn Db) -> Option<FacadeTypeTs<'db>> {
        self.ret_type_js(db).map(Into::into)
    }
}

impl<'db> FacadeParam<'db> {
    pub fn type_ts(self, db: &'db dyn Db) -> FacadeTypeTs<'db> {
        FacadeTypeTs::from(self.type_js(db)).try_as_coerced(db)
    }
}

impl<'db> FacadeFieldPlain<'db> {
    pub fn get_type_ts(self, db: &'db dyn Db) -> FacadeTypeTs<'db> {
        self.type_js(db).into()
    }

    pub fn set_type_ts(self, db: &'db dyn Db) -> FacadeTypeTs<'db> {
        self.get_type_ts(db).try_as_coerced(db)
    }
}
