//! Intermediate representation for the supported subset of Rust's public API.

mod lowering;

use self::lowering::ItemExt;
use crate::compiler::{db::Db, mangle::mangle_path};
use std::{
    fmt,
    path::{Path, PathBuf},
};

#[salsa::interned(debug)]
pub struct ItemPath<'db> {
    pub crate_: Crate<'db>,
    #[returns(ref)]
    pub segments: Vec<String>,
}

#[salsa::tracked]
impl<'db> ItemPath<'db> {
    #[salsa::tracked]
    pub fn from_segment(db: &'db dyn Db, crate_: Crate<'db>, segment: String) -> Self {
        Self::new(db, crate_, vec![segment])
    }

    #[salsa::tracked]
    pub fn append(self, db: &'db dyn Db, segment: String) -> Self {
        let mut segments = self.segments(db).to_owned();
        segments.push(segment.to_owned());
        Self::new(db, self.crate_(db), segments)
    }

    #[salsa::tracked(returns(ref))]
    pub fn fqn(self, db: &'db dyn Db) -> String {
        self.segments(db).join("::")
    }

    #[salsa::tracked(returns(ref))]
    pub fn mangled(self, db: &'db dyn Db) -> String {
        mangle_path(self.segments(db).iter().map(|s| s.as_str()))
    }

    #[salsa::tracked(returns(ref))]
    pub fn name(self, db: &'db dyn Db) -> String {
        self.segments(db)
            .last()
            .expect("Item must have a name")
            .to_string()
    }
}

pub trait AsItem<'db>: Sized {
    fn path(self, db: &'db dyn Db) -> ItemPath<'db>;

    fn fqn(self, db: &'db dyn Db) -> &'db str {
        self.path(db).fqn(db)
    }
}

macro_rules! as_item {
    ($type:ident) => {
        impl<'db> AsItem<'db> for $type<'db> {
            fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
                <$type<'db>>::path(self, db)
            }
        }
    };
}

#[salsa::interned(debug)]
pub struct Crate<'db> {
    #[returns(ref)]
    pub name: String,
}

impl<'db> Crate<'db> {
    pub fn by(db: &'db dyn Db, name: &str) -> Option<Crate<'db>> {
        Self::lower(db, name)
    }

    pub fn root(self, db: &'db dyn Db) -> Mod<'db> {
        self.lower_crate_root(db)
    }

    pub fn dir(self, db: &'db dyn Db) -> &'db Path {
        self.lower_dir(db)
    }
}

#[salsa::interned(debug)]
pub struct Mod<'db> {
    pub parent: ModParent<'db>,
    #[returns(ref)]
    pub name: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum ModParent<'db> {
    Mod(Mod<'db>),
    Crate(Crate<'db>),
}

#[salsa::tracked]
impl<'db> Mod<'db> {
    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        match self.parent(db) {
            ModParent::Mod(parent) => parent.path(db).append(db, self.name(db).to_owned()),
            ModParent::Crate(crate_) => {
                ItemPath::from_segment(db, crate_, self.name(db).to_owned())
            }
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn members(self, db: &'db dyn Db) -> Vec<Member<'db>> {
        self.lower_members(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> &'db Span {
        self.lower_span(db).expect("mod must have span")
    }
}

as_item!(Mod);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum Member<'db> {
    Struct(Struct<'db>),
    Enum(Enum<'db>),
    Function(Function<'db>),
    Mod(Mod<'db>),
}

#[salsa::interned(debug)]
pub struct Struct<'db> {
    pub parent: Mod<'db>,
    #[returns(ref)]
    pub name: String,
}

#[salsa::tracked]
impl<'db> Struct<'db> {
    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.parent(db)
            .path(db)
            .append(db, self.name(db).to_owned())
    }

    pub fn shape(self, db: &'db dyn Db) -> &'db Shape<'db> {
        self.lower_shape(db)
    }

    /// Flattens the shape while omitting private or hidden tuple-field slots.
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<Field<'db>> {
        match self.shape(db) {
            Shape::Unit => vec![],
            Shape::Tuple(fields) => fields.iter().copied().flatten().collect(),
            Shape::Plain { fields, .. } => fields.clone(),
        }
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> &'db Span {
        self.lower_span(db).expect("struct must have span")
    }
}

impl<'db> AsValueType<'db> for Struct<'db> {
    fn as_value_type(self) -> ValueType<'db> {
        ValueType::Struct(self)
    }
}

as_item!(Struct);

#[salsa::interned(debug)]
pub struct Enum<'db> {
    pub parent: Mod<'db>,
    #[returns(ref)]
    pub name: String,
}

#[salsa::tracked]
impl<'db> Enum<'db> {
    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.parent(db)
            .path(db)
            .append(db, self.name(db).to_owned())
    }

    pub fn is_non_exhaustive(self, db: &'db dyn Db) -> bool {
        self.lower_is_non_exhaustive(db)
    }

    pub fn variants(self, db: &'db dyn Db) -> &'db Vec<Variant<'db>> {
        self.lower_variants(db)
    }

    /// Concatenates each variant's visible fields in variant order.
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<Field<'db>> {
        let mut fields = vec![];

        for variant in self.variants(db) {
            fields.extend(variant.fields(db));
        }

        fields
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> &'db Span {
        self.lower_span(db).expect("enum must have span")
    }
}

impl<'db> AsValueType<'db> for Enum<'db> {
    fn as_value_type(self) -> ValueType<'db> {
        ValueType::Enum(self)
    }
}

as_item!(Enum);

#[salsa::interned(debug)]
pub struct Variant<'db> {
    pub parent: Enum<'db>,
    #[returns(ref)]
    pub name: String,
}

#[salsa::tracked]
impl<'db> Variant<'db> {
    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.parent(db)
            .path(db)
            .append(db, self.name(db).to_owned())
    }

    pub fn shape(self, db: &'db dyn Db) -> &'db Shape<'db> {
        self.lower_shape(db)
    }

    /// Flattens the shape while omitting private or hidden tuple-field slots.
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<Field<'db>> {
        match self.shape(db) {
            Shape::Unit => vec![],
            Shape::Tuple(fields) => fields.iter().copied().flatten().collect(),
            Shape::Plain { fields, .. } => fields.clone(),
        }
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> &'db Span {
        self.lower_span(db).expect("variant must have span")
    }
}

as_item!(Variant);

#[derive(Debug, Clone, Hash, PartialEq, Eq, salsa::Update)]
pub enum Shape<'db> {
    Unit,
    /// Tuple fields, where private or hidden fields are represented by `None`.
    Tuple(Vec<Option<Field<'db>>>),
    Plain {
        fields: Vec<Field<'db>>,
        /// Whether private or hidden fields were omitted.
        has_stripped_fields: bool,
    },
}

#[salsa::tracked(debug)]
pub struct Field<'db> {
    pub parent: FieldParent<'db>,
    #[returns(ref)]
    pub name: FieldName,
    #[tracked]
    pub value_type: ValueType<'db>,
    /// Whether generated bindings must prohibit writes to the field.
    #[tracked]
    pub is_readonly: bool,
}

#[salsa::tracked]
impl<'db> Field<'db> {
    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        let name = match self.name(db) {
            FieldName::Idx(idx) => format!("{}", idx),
            FieldName::Name(name) => name.to_owned(),
        };

        match self.parent(db) {
            FieldParent::Struct(struct_) => struct_.path(db).append(db, name),
            FieldParent::Variant(variant) => variant.path(db).append(db, name),
        }
    }

    /// Enum-variant fields are prefixed with their variant to avoid collisions.
    #[salsa::tracked(returns(ref))]
    pub fn mangled_name(self, db: &'db dyn Db) -> String {
        match self.parent(db) {
            FieldParent::Struct(_) => self.name(db).to_string(),
            FieldParent::Variant(variant) => {
                format!("__{}__{}", variant.name(db), self.name(db))
            }
        }
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> &'db Span {
        self.lower_span(db).expect("field must have span")
    }
}

as_item!(Field);

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, salsa::Update, salsa::Supertype)]
pub enum FieldParent<'db> {
    Struct(Struct<'db>),
    Variant(Variant<'db>),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, salsa::Update)]
pub enum FieldName {
    Idx(usize),
    Name(String),
}

impl fmt::Display for FieldName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldName::Idx(idx) => write!(f, "{}", idx),
            FieldName::Name(name) => write!(f, "{}", name),
        }
    }
}

#[salsa::interned(debug)]
pub struct Function<'db> {
    pub parent: Mod<'db>,
    #[returns(ref)]
    pub name: String,
}

#[salsa::tracked]
impl<'db> Function<'db> {
    #[salsa::tracked]
    pub fn path(self, db: &'db dyn Db) -> ItemPath<'db> {
        self.parent(db)
            .path(db)
            .append(db, self.name(db).to_owned())
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<Param<'db>> {
        self.lower_params(db)
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Option<ValueType<'db>> {
        self.lower_ret_type(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.lower_docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> &'db Span {
        self.lower_span(db).expect("function must have span")
    }
}

as_item!(Function);

#[salsa::tracked(debug)]
pub struct Param<'db> {
    pub parent: Function<'db>,
    pub index: usize,
    #[returns(ref)]
    #[tracked]
    pub name: String,
    #[returns(ref)]
    #[tracked]
    pub type_: Type<'db>,
}

/// A supported subset of Rust value types.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ValueType<'db> {
    bool,
    char,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    f32,
    f64,
    Str,
    String,
    Struct(Struct<'db>),
    Enum(Enum<'db>),
}

impl<'db> ValueType<'db> {
    pub fn fqn(self, db: &'db dyn Db) -> &'db str {
        match self {
            ValueType::bool => "bool",
            ValueType::char => "char",
            ValueType::i8 => "i8",
            ValueType::u8 => "u8",
            ValueType::i16 => "i16",
            ValueType::u16 => "u16",
            ValueType::i32 => "i32",
            ValueType::u32 => "u32",
            ValueType::f32 => "f32",
            ValueType::f64 => "f64",
            ValueType::Str => "str",
            ValueType::String => "String",
            ValueType::Struct(s) => s.fqn(db),
            ValueType::Enum(e) => e.fqn(db),
        }
    }

    pub fn borrowed(self) -> Type<'db> {
        Type::Borrowed {
            is_mutable: false,
            inner: self,
        }
    }

    pub fn borrowed_mut(self) -> Type<'db> {
        Type::Borrowed {
            is_mutable: true,
            inner: self,
        }
    }

    pub fn as_type(self) -> Type<'db> {
        Type::Value(self)
    }
}

impl<'db> From<ValueType<'db>> for Type<'db> {
    fn from(value: ValueType<'db>) -> Self {
        Type::Value(value)
    }
}

pub trait AsValueType<'db> {
    fn as_value_type(self) -> ValueType<'db>;
}

impl<'db, T: AsValueType<'db>> From<T> for ValueType<'db> {
    fn from(value: T) -> Self {
        value.as_value_type()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Type<'db> {
    Value(ValueType<'db>),
    Borrowed {
        is_mutable: bool,
        inner: ValueType<'db>,
    },
}

impl<'db> Type<'db> {
    pub fn deborrow(self) -> ValueType<'db> {
        match self {
            Type::Value(value_type) => value_type,
            Type::Borrowed { inner, .. } => inner,
        }
    }
}

pub trait AsType<'db> {
    fn as_type(self) -> Type<'db>;
}

impl<'db, T: AsValueType<'db>> AsType<'db> for T {
    fn as_type(self) -> Type<'db> {
        Type::Value(self.as_value_type())
    }
}

impl<'db, T: AsType<'db>> From<T> for Type<'db> {
    fn from(value: T) -> Self {
        value.as_type()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Span {
    pub abs_path: PathBuf,
    /// The path reachable from the crate root.
    ///
    /// This may be relative for files in the main crate and absolute otherwise.
    pub src_path: PathBuf,
    pub begin: (usize, usize),
    pub end: (usize, usize),
}
