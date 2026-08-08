use crate::compiler::{
    db::Db,
    ir::{
        ffi::{FfiClass, FfiCrate, FfiFunction, FfiParam, FfiType},
        rust::{Crate, Enum, Field, Function, Mod, Struct, Type, ValueType},
    },
};

#[salsa::interned(debug)]
pub struct Shim<'db> {
    #[returns(ref)]
    pub crates: Vec<ShimCrate<'db>>,
}

#[salsa::interned(debug)]
pub struct ShimCrate<'db> {
    pub src: Crate<'db>,
}

impl<'db> ShimCrate<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiCrate<'db> {
        FfiCrate::new(db, self)
    }

    pub fn mods(self, db: &'db dyn Db) -> &Vec<ShimMod<'db>> {
        todo!()
    }
}

#[salsa::interned(debug)]
pub struct ShimMod<'db> {
    pub src: Mod<'db>,
}

impl<'db> ShimMod<'db> {
    pub fn classes(self, db: &'db dyn Db) -> &Vec<ShimClass<'db>> {
        todo!()
    }

    pub fn fns(self, db: &'db dyn Db) -> &Vec<ShimFunction<'db>> {
        todo!()
    }
}

#[salsa::interned(debug)]
pub struct ShimClass<'db> {
    pub src: ShimClassSource<'db>,
}

#[salsa::tracked]
impl<'db> ShimClass<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiClass<'db> {
        FfiClass::new(db, self)
    }

    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        match self.src(db) {
            ShimClassSource::Struct(s) => format!("__struct__{}", s.path(db).mangled(db)),
            ShimClassSource::Enum(e) => format!("__enum__{}", e.path(db).mangled(db)),
        }
    }

    pub fn fields(self, db: &'db dyn Db) -> &Vec<ShimField<'db>> {
        todo!()
    }
}

impl<'db> From<ShimClass<'db>> for ShimValueType<'db> {
    fn from(value: ShimClass<'db>) -> Self {
        ShimValueType::Class(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
enum ShimClassSource<'db> {
    Struct(Struct<'db>),
    Enum(Enum<'db>),
}

impl<'db> From<ShimClassSource<'db>> for ValueType<'db> {
    fn from(value: ShimClassSource<'db>) -> Self {
        match value {
            ShimClassSource::Struct(struct_) => struct_.into(),
            ShimClassSource::Enum(enum_) => enum_.into(),
        }
    }
}

#[salsa::interned(debug)]
pub struct ShimField<'db> {
    pub src: Field<'db>,
}

impl<'db> ShimField<'db> {
    pub fn name(self, db: &'db dyn Db) -> &str {
        todo!()
    }

    pub fn getter(self, db: &'db dyn Db) -> ShimFunction<'db> {
        todo!()
    }

    pub fn setter(self, db: &'db dyn Db) -> Option<ShimFunction<'db>> {
        todo!()
    }
}

#[salsa::interned(debug)]
pub struct ShimFunction<'db> {
    pub kind: ShimFunctionKind<'db>,
}

impl<'db> ShimFunction<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, self)
    }

    pub fn symbol(self, db: &'db dyn Db) -> &str {
        todo!()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimValueType<'db>> {
        todo!()
    }

    pub fn params(self, db: &'db dyn Db) -> &Vec<ShimParam<'db>> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ShimFunctionKind<'db> {
    Free(ShimFunctionFree<'db>),
    Getter(ShimFunctionGetter<'db>),
    Setter(ShimFunctionSetter<'db>),
}

#[salsa::interned(debug)]
pub struct ShimFunctionFree<'db> {
    pub src: Function<'db>,
}

impl<'db> ShimFunctionFree<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        todo!()
    }

    pub fn symbol(self, db: &'db dyn Db) -> &str {
        todo!()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimValueType<'db>> {
        todo!()
    }

    pub fn params(self, db: &'db dyn Db) -> &Vec<ShimParam<'db>> {
        todo!()
    }
}

#[salsa::tracked(debug)]
pub struct ShimParam<'db> {
    pub parent: ShimFunction<'db>,
    pub idx: usize,
    #[tracked]
    #[returns(ref)]
    pub name: String,
    #[tracked]
    pub src_type: Type<'db>,
    #[tracked]
    pub type_: ShimType<'db>,
}

impl<'db> ShimParam<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiParam<'db> {
        FfiParam::new(db, self)
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionGetter<'db> {
    pub src: Field<'db>,
}

impl<'db> ShimFunctionGetter<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        todo!()
    }

    pub fn symbol(self, db: &'db dyn Db) -> &str {
        todo!()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> ShimValueType<'db> {
        todo!()
    }

    pub fn class(self, db: &'db dyn Db) -> ShimClass<'db> {
        todo!()
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionSetter<'db> {
    pub src: Field<'db>,
}

impl<'db> ShimFunctionSetter<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        todo!()
    }

    pub fn symbol(self, db: &'db dyn Db) -> &str {
        todo!()
    }

    pub fn class(self, db: &'db dyn Db) -> ShimClass<'db> {
        todo!()
    }

    pub fn set_type(self, db: &'db dyn Db) -> ShimType<'db> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum ShimValueType<'db> {
    bool,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    f64,
    String,
    JsAny,
    Class(ShimClass<'db>),
}

impl<'db> ShimValueType<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiType<'db> {
        todo!()
    }

    pub fn borrowed(self) -> ShimType<'db> {
        ShimType::Borrowed(self)
    }
}

impl<'db> From<ShimValueType<'db>> for ShimType<'db> {
    fn from(value: ShimValueType<'db>) -> Self {
        ShimType::Value(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ShimType<'db> {
    Value(ShimValueType<'db>),
    Borrowed(ShimValueType<'db>),
}

impl<'db> ShimType<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiType<'db> {
        todo!()
    }

    pub fn src(self, db: &'db dyn Db) -> ValueType<'db> {
        todo!()
    }

    pub fn deborrow(self) -> ShimValueType<'db> {
        match self {
            ShimType::Value(value_type) => value_type,
            ShimType::Borrowed(inner) => inner,
        }
    }

    pub fn symbol(self, db: &'db dyn Db) -> &String {
        todo!()
    }
}
