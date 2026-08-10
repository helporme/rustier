use crate::compiler::{
    db::Db,
    ir::{
        ffi::{FfiClass, FfiCrate, FfiFunction, FfiParam, FfiType},
        rust::{Crate, Enum, Field, FieldParent, Function, Member, Mod, Struct, Type, ValueType},
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

#[salsa::tracked]
impl<'db> ShimCrate<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiCrate<'db> {
        FfiCrate::new(db, self)
    }

    #[salsa::tracked(returns(ref))]
    pub fn mods(self, db: &'db dyn Db) -> Vec<ShimMod<'db>> {
        let mut mods = vec![];
        let mut wave = vec![];

        let root = self.src(db).root(db);
        wave.push(root);
        mods.push(root);

        while let Some(mod_) = wave.pop() {
            for member in mod_.members(db) {
                if let Member::Mod(sub) = member {
                    wave.push(*sub);
                    mods.push(*sub);
                }
            }
        }

        mods.into_iter().map(|m| ShimMod::new(db, m)).collect()
    }
}

#[salsa::interned(debug)]
pub struct ShimMod<'db> {
    pub src: Mod<'db>,
}

#[salsa::tracked]
impl<'db> ShimMod<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn classes(self, db: &'db dyn Db) -> Vec<ShimClass<'db>> {
        self.src(db)
            .members(db)
            .iter()
            .filter_map(|mem| match mem {
                Member::Struct(s) => Some(s.shim(db)),
                Member::Enum(e) => Some(e.shim(db)),
                _ => None,
            })
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub fn fns(self, db: &'db dyn Db) -> Vec<ShimFunction<'db>> {
        let mut fns = vec![];

        for mem in self.src(db).members(db).iter() {
            if let Member::Function(function) = mem {
                fns.push(ShimFunction::new(
                    db,
                    ShimFunctionKind::Free(ShimFunctionFree::new(db, function)),
                ));
            } else {
                let cls = match mem {
                    Member::Struct(s) => Some(s.shim(db)),
                    Member::Enum(e) => Some(ShimClass::new(db, ShimClassSource::Enum(*e))),
                    _ => None,
                };

                if let Some(cls) = cls {
                    for field in cls.fields(db) {
                        fns.push(field.getter(db));
                        if let Some(f) = field.setter(db) {
                            fns.push(f);
                        }
                    }
                }
            }
        }

        fns
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

    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<ShimField<'db>> {
        let fields = match self.src(db) {
            ShimClassSource::Struct(s) => s.fields(db),
            ShimClassSource::Enum(e) => e.fields(db),
        };

        fields.iter().map(|f| ShimField::new(db, f)).collect()
    }

    pub fn as_value_type(self, db: &'db dyn Db) -> ShimValueType<'db> {
        ShimValueType {
            kind: ShimValueTypeKind::Class(self),
            src: self.src(db).into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ShimClassSource<'db> {
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
    pub fn getter(self, db: &'db dyn Db) -> ShimFunction<'db> {
        ShimFunction::new(
            db,
            ShimFunctionKind::Getter(ShimFunctionGetter::new(db, self.src(db))),
        )
    }

    pub fn setter(self, db: &'db dyn Db) -> Option<ShimFunction<'db>> {
        let src = self.src(db);

        src.is_readonly(db).then(|| {
            ShimFunction::new(
                db,
                ShimFunctionKind::Setter(ShimFunctionSetter::new(db, src)),
            )
        })
    }
}

#[salsa::interned(debug)]
pub struct ShimFunction<'db> {
    pub kind: ShimFunctionKind<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunction<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        FfiFunction::new(db, self)
    }

    pub fn symbol(self, db: &'db dyn Db) -> &'db str {
        let kind = self.kind(db);

        match kind {
            ShimFunctionKind::Free(f) => f.symbol(db),
            ShimFunctionKind::Getter(g) => g.symbol(db),
            ShimFunctionKind::Setter(s) => s.symbol(db),
            ShimFunctionKind::EnumDiscriminator(e) => e.symbol(db),
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimValueType<'db>> {
        let kind = self.kind(db);

        match kind {
            ShimFunctionKind::Free(f) => f.ret_type(db),
            ShimFunctionKind::Getter(g) => Some(g.ret_type(db)),
            ShimFunctionKind::Setter(_) => None,
            ShimFunctionKind::EnumDiscriminator(d) => Some(d.ret_type()),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<ShimParam<'db>> {
        let kind = self.kind(db);

        match kind {
            ShimFunctionKind::Free(f) => f.params(db).clone(),
            ShimFunctionKind::Getter(g) => g.params(db).to_vec(),
            ShimFunctionKind::Setter(s) => s.params(db).to_vec(),
            ShimFunctionKind::EnumDiscriminator(e) => e.params(db).to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ShimFunctionKind<'db> {
    Free(ShimFunctionFree<'db>),
    Getter(ShimFunctionGetter<'db>),
    Setter(ShimFunctionSetter<'db>),
    EnumDiscriminator(ShimFunctionEnumDiscriminator<'db>),
}

#[salsa::interned(debug)]
pub struct ShimFunctionFree<'db> {
    pub src: Function<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionFree<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        let fun = ShimFunction::new(db, ShimFunctionKind::Free(self));
        FfiFunction::new(db, fun)
    }

    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        let src = self.src(db);
        format!("__fn__free__{}", src.path(db).mangled(db))
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ShimValueType<'db>> {
        self.src(db).ret_type(db).map(|r| r.shim_out(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<ShimParam<'db>> {
        let fun = ShimFunction::new(db, ShimFunctionKind::Free(self));

        self.src(db)
            .params(db)
            .iter()
            .enumerate()
            .map(|(idx, src)| {
                let src_type = src.type_(db);
                let shim_type = src_type.shim_in(db);
                ShimParam::new(db, fun, idx, *src_type, shim_type)
            })
            .collect()
    }
}

#[salsa::tracked(debug)]
pub struct ShimParam<'db> {
    pub parent: ShimFunction<'db>,
    pub idx: usize,
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

#[salsa::tracked]
impl<'db> ShimFunctionGetter<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        let fun = ShimFunction::new(db, ShimFunctionKind::Getter(self));
        FfiFunction::new(db, fun)
    }

    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        let src = self.src(db);
        let parent_name = match src.parent(db) {
            FieldParent::Struct(s) => s.path(db).mangled(db),
            FieldParent::Variant(v) => v.parent(db).path(db).mangled(db),
        };
        let field_name = src.mangled_name(db);

        format!("__fn__getter__{}__{}", parent_name, field_name)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> ShimValueType<'db> {
        let field = self.src(db);
        field.type_(db).shim_out(db)
    }

    pub fn class(self, db: &'db dyn Db) -> ShimClass<'db> {
        let field = self.src(db);
        match field.parent(db) {
            FieldParent::Struct(s) => s.shim(db),
            FieldParent::Variant(v) => v.parent(db).shim(db),
        }
    }

    #[salsa::tracked]
    pub fn params(self, db: &'db dyn Db) -> [ShimParam<'db>; 1] {
        let field = self.src(db);

        let src_type: ValueType<'db> = match field.parent(db) {
            FieldParent::Struct(s) => s.into(),
            FieldParent::Variant(v) => v.parent(db).into(),
        };

        let fun = ShimFunction::new(db, ShimFunctionKind::Getter(self));

        [ShimParam::new(
            db,
            fun,
            0,
            src_type.into(),
            src_type.shim_in(db),
        )]
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionSetter<'db> {
    pub src: Field<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionSetter<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        let fun = ShimFunction::new(db, ShimFunctionKind::Setter(self));
        FfiFunction::new(db, fun)
    }

    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        let src = self.src(db);
        let parent_name = match src.parent(db) {
            FieldParent::Struct(s) => s.path(db).mangled(db),
            FieldParent::Variant(v) => v.parent(db).path(db).mangled(db),
        };
        let field_name = src.mangled_name(db);

        format!("__fn__setter__{}__{}", parent_name, field_name)
    }

    pub fn class(self, db: &'db dyn Db) -> ShimClass<'db> {
        let field = self.src(db);
        match field.parent(db) {
            FieldParent::Struct(s) => s.shim(db),
            FieldParent::Variant(v) => v.parent(db).shim(db),
        }
    }

    pub fn set_type(self, db: &'db dyn Db) -> ShimType<'db> {
        self.src(db).type_(db).shim_in(db)
    }

    #[salsa::tracked]
    pub fn params(self, db: &'db dyn Db) -> [ShimParam<'db>; 2] {
        let field = self.src(db);

        let src_type: ValueType<'db> = match field.parent(db) {
            FieldParent::Struct(s) => s.into(),
            FieldParent::Variant(v) => v.parent(db).into(),
        };
        let value_type = field.type_(db);

        let fun = ShimFunction::new(db, ShimFunctionKind::Setter(self));

        [
            ShimParam::new(db, fun, 0, src_type.into(), src_type.shim_in(db)),
            ShimParam::new(db, fun, 1, value_type.into(), value_type.shim_in(db)),
        ]
    }
}

#[salsa::interned(debug)]
pub struct ShimFunctionEnumDiscriminator<'db> {
    pub src: Enum<'db>,
}

#[salsa::tracked]
impl<'db> ShimFunctionEnumDiscriminator<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiFunction<'db> {
        let fun = ShimFunction::new(db, ShimFunctionKind::EnumDiscriminator(self));
        FfiFunction::new(db, fun)
    }

    #[salsa::tracked(returns(ref))]
    pub fn symbol(self, db: &'db dyn Db) -> String {
        let src = self.src(db);
        format!("__fn__disc__{}", src.path(db).mangled(db))
    }

    pub fn ret_type(self) -> ShimValueType<'db> {
        ShimValueType {
            kind: ShimValueTypeKind::u32,
            src: ValueType::u32,
        }
    }

    pub fn class(self, db: &'db dyn Db) -> ShimClass<'db> {
        self.src(db).shim(db)
    }

    #[salsa::tracked]
    pub fn params(self, db: &'db dyn Db) -> [ShimParam<'db>; 1] {
        let src = self.src(db);
        let fun = ShimFunction::new(db, ShimFunctionKind::EnumDiscriminator(self));
        let src_type = Type::from(ValueType::from(src));

        [ShimParam::new(db, fun, 0, src_type, src_type.shim_in(db))]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum ShimValueTypeKind<'db> {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub struct ShimValueType<'db> {
    pub kind: ShimValueTypeKind<'db>,
    pub src: ValueType<'db>,
}

impl<'db> ShimValueType<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiType<'db> {
        match self.src {
            ValueType::bool => FfiType::bool,
            ValueType::char => FfiType::u32,
            ValueType::i8 => FfiType::i8,
            ValueType::u8 => FfiType::u8,
            ValueType::i16 => FfiType::i16,
            ValueType::u16 => FfiType::u16,
            ValueType::i32 => FfiType::i32,
            ValueType::u32 => FfiType::u32,
            ValueType::f32 | ValueType::f64 => FfiType::f64,
            ValueType::Str | ValueType::String => FfiType::String,
            ValueType::Struct(s) => FfiType::Class(s.shim(db).ffi(db)),
            ValueType::Enum(e) => FfiType::Class(e.shim(db).ffi(db)),
        }
    }

    pub fn borrowed(self) -> ShimTypeKind<'db> {
        ShimTypeKind::Borrowed(self)
    }
}

impl<'db> From<ShimValueType<'db>> for ShimType<'db> {
    fn from(value: ShimValueType<'db>) -> Self {
        ShimType {
            kind: ShimTypeKind::Value(value),
            src: Type::Value(value.src),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct ShimType<'db> {
    pub kind: ShimTypeKind<'db>,
    pub src: Type<'db>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ShimTypeKind<'db> {
    Value(ShimValueType<'db>),
    Borrowed(ShimValueType<'db>),
}

impl<'db> ShimType<'db> {
    pub fn ffi(self, db: &'db dyn Db) -> FfiType<'db> {
        self.deborrow().ffi(db)
    }

    pub fn deborrow(self) -> ShimValueType<'db> {
        match self.kind {
            ShimTypeKind::Value(ty) => ty,
            ShimTypeKind::Borrowed(ty) => ty,
        }
    }
}

impl<'db> Struct<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimClass<'db> {
        ShimClass::new(db, ShimClassSource::Struct(self))
    }
}

impl<'db> Enum<'db> {
    pub fn shim(self, db: &'db dyn Db) -> ShimClass<'db> {
        ShimClass::new(db, ShimClassSource::Enum(self))
    }
}

impl<'db> ValueType<'db> {
    pub fn shim_in(self, db: &'db dyn Db) -> ShimType<'db> {
        match self {
            ValueType::bool
            | ValueType::char
            | ValueType::i8
            | ValueType::u8
            | ValueType::i16
            | ValueType::u16
            | ValueType::i32
            | ValueType::u32
            | ValueType::f32
            | ValueType::f64
            | ValueType::Str
            | ValueType::String => ShimType {
                kind: ShimTypeKind::Value(ShimValueType {
                    kind: ShimValueTypeKind::JsAny,
                    src: self,
                }),
                src: self.into(),
            },
            ValueType::Struct(s) => ShimType {
                kind: ShimTypeKind::Borrowed(ShimValueType {
                    kind: ShimValueTypeKind::Class(s.shim(db)),
                    src: self,
                }),
                src: self.into(),
            },
            ValueType::Enum(e) => ShimType {
                kind: ShimTypeKind::Borrowed(ShimValueType {
                    kind: ShimValueTypeKind::Class(e.shim(db)),
                    src: self,
                }),
                src: self.into(),
            },
        }
    }

    pub fn shim_out(self, db: &'db dyn Db) -> ShimValueType<'db> {
        match self {
            ValueType::bool => ShimValueType {
                kind: ShimValueTypeKind::bool,
                src: self.into(),
            },
            ValueType::char => ShimValueType {
                kind: ShimValueTypeKind::u32,
                src: self.into(),
            },
            ValueType::i8 => ShimValueType {
                kind: ShimValueTypeKind::i8,
                src: self.into(),
            },
            ValueType::u8 => ShimValueType {
                kind: ShimValueTypeKind::u8,
                src: self.into(),
            },
            ValueType::i16 => ShimValueType {
                kind: ShimValueTypeKind::i16,
                src: self.into(),
            },
            ValueType::u16 => ShimValueType {
                kind: ShimValueTypeKind::u16,
                src: self.into(),
            },
            ValueType::i32 => ShimValueType {
                kind: ShimValueTypeKind::i32,
                src: self.into(),
            },
            ValueType::u32 => ShimValueType {
                kind: ShimValueTypeKind::u32,
                src: self.into(),
            },
            ValueType::f32 => ShimValueType {
                kind: ShimValueTypeKind::f64,
                src: self.into(),
            },
            ValueType::f64 => ShimValueType {
                kind: ShimValueTypeKind::f64,
                src: self.into(),
            },
            ValueType::Str => ShimValueType {
                kind: ShimValueTypeKind::String,
                src: self.into(),
            },
            ValueType::String => ShimValueType {
                kind: ShimValueTypeKind::String,
                src: self.into(),
            },
            ValueType::Struct(s) => ShimValueType {
                kind: ShimValueTypeKind::Class(s.shim(db)),
                src: self.into(),
            },
            ValueType::Enum(e) => ShimValueType {
                kind: ShimValueTypeKind::Class(e.shim(db)),
                src: self.into(),
            },
        }
    }
}

impl<'db> Type<'db> {
    pub fn shim_in(self, db: &'db dyn Db) -> ShimType<'db> {
        self.deborrow().shim_in(db)
    }

    pub fn shim_out(self, db: &'db dyn Db) -> ShimValueType<'db> {
        self.deborrow().shim_out(db)
    }
}
