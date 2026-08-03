//! Intermediate representation used by code generation.

use std::path::PathBuf;

use crate::codegen::{db::Db, mangle::mangle_path};

macro_rules! define_item_id {
    ($name:ident, $kind:path, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, salsa::Update)]
        pub struct $name<'db>($crate::codegen::ir::ItemId<'db>);

        impl<'db> From<$name<'db>> for $crate::codegen::ir::ItemId<'db> {
            fn from(value: $name<'db>) -> Self {
                value.0
            }
        }

        impl<'db> $crate::codegen::ir::IntoItemId<'db> for $name<'db> {
            fn into_item_id(
                self,
                _db: &'db dyn $crate::codegen::db::Db,
            ) -> $crate::codegen::ir::ItemId<'db> {
                self.0
            }
        }

        impl<'db> $name<'db> {
            /// Creates an ID when the inner item has the expected kind.
            // todo: rename or change to something more useful
            pub fn new(
                db: &'db dyn $crate::codegen::db::Db,
                inner: $crate::codegen::ir::ItemId<'db>,
            ) -> Option<Self> {
                if inner.kind(db) == $kind {
                    Some(Self(inner))
                } else {
                    None
                }
            }
        }
    };
}

/// Fully qualified item path.
#[salsa::interned(debug)]
pub struct ItemPath<'db> {
    #[returns(ref)]
    pub segments: Vec<String>,
}

/// Item kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum ItemKind {
    Struct,
    Enum,
    CLikeEnum,
    Mod,
    Fn,
}

/// Identifies a Rust item by its fully qualified path.
#[salsa::interned(debug)]
#[derive(Ord, PartialOrd)]
pub struct ItemId<'db> {
    /// Fully qualified item path including a crate name.
    #[returns(ref)]
    pub path: ItemPath<'db>,
    /// The kind of the item.
    #[returns(copy)]
    pub kind: ItemKind,
}

#[salsa::tracked]
impl<'db> ItemId<'db> {
    /// Returns the fully qualified item name, e.g. `geometry::Point`.
    #[salsa::tracked(returns(ref))]
    pub fn fqn(self, db: &'db dyn Db) -> String {
        self.path(db).segments(db).join("::")
    }

    /// Returns the unqualified item name, e.g. `Point`.
    #[salsa::tracked(returns(ref))]
    pub fn name(self, db: &'db dyn Db) -> String {
        self.path(db)
            .segments(db)
            .last()
            .expect("Item must have a name")
            .to_string()
    }

    /// Returns the mangled fully qualified item name, e.g. `geometry_Point`.
    #[salsa::tracked(returns(ref))]
    pub fn mangled_fqn(self, db: &'db dyn Db) -> String {
        mangle_path(self.path(db).segments(db))
    }

    /// Returns the item's exported JavaScript name.
    #[salsa::tracked(returns(ref))]
    pub fn extern_name(self, db: &'db dyn Db) -> String {
        format!("__ext_{}", self.mangled_fqn(db))
    }

    /// Returns the parent module ID, or `None` if this ID has no parent path
    /// segment.
    #[salsa::tracked]
    pub fn parent_mod_id(self, db: &'db dyn Db) -> Option<ModId<'db>> {
        let segments = &self.path(db).segments(db);

        if segments.len() <= 1 {
            return None;
        }

        // todo: result of this function invokation is always unwrapped since it's used
        // in a context where it's obvious that the function returns Some (like id of
        // fn, struct etc.). It might be worth considering adding a special ids like
        // FnId to avoid it.
        Some(
            ModId::new(
                db,
                ItemId::new(
                    db,
                    ItemPath::new(db, segments[..segments.len() - 1].to_vec()),
                    ItemKind::Mod,
                ),
            )
            .unwrap(),
        )
    }
}

impl<'db> IntoItemId<'db> for ItemId<'db> {
    fn into_item_id(self, _db: &'db dyn Db) -> ItemId<'db> {
        self
    }
}

/// A trait for converting an item into an [`ItemId`].
pub trait IntoItemId<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db>;
}

/// Provides common item ID operations over [`ItemId`].
pub trait ItemIdExt<'db> {
    /// Fully qualified item name, e.g. `kitties::FluffyCat`.
    fn fqn(self, db: &'db dyn Db) -> &'db String;

    /// Unqualified item name, e.g. `FluffyCat`.
    fn name(self, db: &'db dyn Db) -> &'db String;

    /// Item's exported JavaScript name.
    fn extern_name(self, db: &'db dyn Db) -> &'db String;

    /// Parent module ID, or `None` when the item has no parent.
    fn parent_mod_id(self, db: &'db dyn Db) -> Option<ModId<'db>>;
}

impl<'db, T: IntoItemId<'db>> ItemIdExt<'db> for T {
    fn fqn(self, db: &'db dyn Db) -> &'db String {
        self.into_item_id(db).fqn(db)
    }

    fn name(self, db: &'db dyn Db) -> &'db String {
        self.into_item_id(db).name(db)
    }

    fn extern_name(self, db: &'db dyn Db) -> &'db String {
        self.into_item_id(db).extern_name(db)
    }

    fn parent_mod_id(self, db: &'db dyn Db) -> Option<ModId<'db>> {
        self.into_item_id(db).parent_mod_id(db)
    }
}

/// A lowered Rust item stored in the IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Supertype, salsa::Update)]
pub enum Item<'db> {
    CLikeEnum(CLikeEnum<'db>),
    Enum(Enum<'db>),
    Fn(Fn<'db>),
    Mod(Mod<'db>),
    Struct(Struct<'db>),
}

impl<'db> From<CLikeEnum<'db>> for Item<'db> {
    fn from(value: CLikeEnum<'db>) -> Self {
        Self::CLikeEnum(value)
    }
}

impl<'db> From<Enum<'db>> for Item<'db> {
    fn from(value: Enum<'db>) -> Self {
        Self::Enum(value)
    }
}

impl<'db> From<Fn<'db>> for Item<'db> {
    fn from(value: Fn<'db>) -> Self {
        Self::Fn(value)
    }
}

impl<'db> From<Mod<'db>> for Item<'db> {
    fn from(value: Mod<'db>) -> Self {
        Self::Mod(value)
    }
}

impl<'db> From<Struct<'db>> for Item<'db> {
    fn from(value: Struct<'db>) -> Self {
        Self::Struct(value)
    }
}

impl<'db> IntoItemId<'db> for Item<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        match self {
            Self::CLikeEnum(item) => item.into_item_id(db),
            Self::Enum(item) => item.into_item_id(db),
            Self::Fn(item) => item.into_item_id(db),
            Self::Mod(item) => item.into_item_id(db),
            Self::Struct(item) => item.into_item_id(db),
        }
    }
}

/// A source code span as one-based row/column bounds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Span {
    // note: split on two paths is a preparation for supporting external crates, however currently
    // it doesn't make much sense.
    /// The absolute path to the source file.
    pub abs_path: PathBuf,
    /// The path rechable from the crate root.
    ///
    /// It might be a relative path if the span is pointing to the file inside
    /// the "main" crate, or an absolute path otherwise.
    pub src_path: PathBuf,
    pub begin: (usize, usize),
    pub end: (usize, usize),
}

/// A field declared by a struct or enum variant.
#[salsa::tracked(debug)]
pub struct ShapeField<'db> {
    #[returns(ref)]
    pub name: String,
    #[tracked]
    pub type_: ValueType<'db>,
    #[tracked]
    pub is_readonly: bool,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

#[salsa::tracked]
impl<'db> ShapeField<'db> {
    /// Returns the struct field binding getter function name, e.g.
    /// `__getter__foo`.
    #[salsa::tracked(returns(ref))]
    pub fn getter_binding_name_on_struct(self, db: &'db dyn Db) -> String {
        format!("__getter__{}", self.name(db))
    }

    /// Returns the struct field binding setter function name, e.g.
    /// `__setter__foo`.
    #[salsa::tracked(returns(ref))]
    pub fn setter_binding_name_on_struct(self, db: &'db dyn Db) -> String {
        format!("__setter__{}", self.name(db))
    }
}

/// The field layout shared by structs and enum variants.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum Shape<'db> {
    Unit,
    /// Tuple fields, where private or hidden fields are represented by `None`.
    Tuple(Vec<Option<ShapeField<'db>>>),
    Plain {
        fields: Vec<ShapeField<'db>>,
        has_stripped_fields: bool,
    },
}

impl<'db> Shape<'db> {
    /// Iterates over the public lowered fields in their declaration order.
    pub fn fields(&self) -> impl Iterator<Item = ShapeField<'db>> + Clone + '_ {
        let (tuple_fields, plain_fields): (&[Option<ShapeField<'db>>], &[ShapeField<'db>]) =
            match self {
                Self::Unit => (&[], &[]),
                Self::Tuple(fields) => (fields, &[]),
                Self::Plain { fields, .. } => (&[], fields),
            };

        tuple_fields.iter().flatten().chain(plain_fields).copied()
    }
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
    Struct(StructId<'db>),
    Enum(EnumId<'db>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Type<'db> {
    Value(ValueType<'db>),
    Borrowed {
        is_mutable: bool,
        // note that we do not support double refs
        inner: ValueType<'db>,
    },
}

impl<'db> Type<'db> {
    pub fn deref(self) -> ValueType<'db> {
        match self {
            Type::Value(value) => value,
            Type::Borrowed { inner, .. } => inner,
        }
    }
}

impl<'db> ValueType<'db> {
    /// Returns this type's TypeScript name.
    pub fn ts_name(&self, db: &'db dyn Db) -> String {
        match self {
            ValueType::bool => "boolean".to_owned(),
            ValueType::char
            | ValueType::i8
            | ValueType::u8
            | ValueType::i16
            | ValueType::u16
            | ValueType::i32
            | ValueType::u32
            | ValueType::f32
            | ValueType::f64 => "number".to_owned(),
            ValueType::Str | ValueType::String => "string".to_owned(),
            ValueType::Struct(id) => id.name(db).to_owned(),
            ValueType::Enum(id) => id.name(db).to_owned(),
        }
    }
}

pub trait IntoIrTypeExt<'db> {
    fn into_ir_type(self, db: &'db dyn Db) -> ValueType<'db>;
}

impl<'db, T: IntoItemId<'db>> IntoIrTypeExt<'db> for T {
    fn into_ir_type(self, db: &'db dyn Db) -> ValueType<'db> {
        let item_id = self.into_item_id(db);

        match item_id.kind(db) {
            ItemKind::Struct => ValueType::Struct(StructId::new(db, item_id).unwrap()),
            ItemKind::Enum => ValueType::Enum(EnumId::new(db, item_id).unwrap()),
            ItemKind::CLikeEnum => ValueType::Enum(EnumId::new(db, item_id).unwrap()),
            ItemKind::Mod => ValueType::Enum(EnumId::new(db, item_id).unwrap()),
            ItemKind::Fn => ValueType::Enum(EnumId::new(db, item_id).unwrap()),
        }
    }
}

define_item_id!(FnId, ItemKind::Fn, "Identifies a lowered Rust function.");

/// A function.
#[salsa::tracked(debug)]
pub struct Fn<'db> {
    pub id: FnId<'db>,
    #[tracked]
    #[returns(ref)]
    pub params: Vec<FnParam<'db>>,
    #[tracked]
    #[returns(copy)]
    pub return_type: Option<ValueType<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

impl<'db> IntoItemId<'db> for Fn<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        self.id(db).into()
    }
}

/// A function parameter.
#[salsa::tracked(debug)]
pub struct FnParam<'db> {
    #[returns(ref)]
    pub name: String,
    #[tracked]
    pub type_: Type<'db>,
}

define_item_id!(
    StructId,
    ItemKind::Struct,
    "Identifies a lowered Rust struct."
);

/// A lowered Rust struct and its fields.
#[salsa::tracked(debug)]
pub struct Struct<'db> {
    pub id: StructId<'db>,
    #[tracked]
    #[returns(ref)]
    pub shape: Shape<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

#[salsa::tracked]
impl<'db> Struct<'db> {
    /// Returns the public lowered fields in declaration order.
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<ShapeField<'db>> {
        // todo: make .fields() a tracked function instead of this wrapper
        self.shape(db).fields().collect()
    }
}

impl<'db> IntoItemId<'db> for Struct<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        self.id(db).into()
    }
}

define_item_id!(EnumId, ItemKind::Enum, "Identifies a lowered Rust enum.");

/// A lowered Rust enum and its variants.
#[salsa::tracked(debug)]
pub struct Enum<'db> {
    pub id: EnumId<'db>,
    #[tracked]
    #[returns(ref)]
    pub variants: Vec<EnumVariant<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
    #[tracked]
    pub is_non_exhaustive: bool,
}

impl<'db> IntoItemId<'db> for Enum<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        self.id(db).into()
    }
}

/// A lowered enum variant and its field layout.
#[salsa::tracked(debug)]
pub struct EnumVariant<'db> {
    #[returns(ref)]
    pub name: String,
    #[tracked]
    #[returns(ref)]
    pub shape: Shape<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

impl<'db> EnumVariant<'db> {
    /// Iterates over this variant's public lowered fields in declaration order.
    pub fn fields(
        self,
        db: &'db dyn crate::codegen::db::Db,
    ) -> impl Iterator<Item = ShapeField<'db>> + Clone + 'db {
        self.shape(db).fields()
    }
}

define_item_id!(
    CLikeEnumId,
    ItemKind::CLikeEnum,
    "Identifies a lowered Rust c-like enum."
);

/// A lowered Rust enum that contains only unit variants.
#[salsa::tracked(debug)]
pub struct CLikeEnum<'db> {
    pub id: CLikeEnumId<'db>,
    #[tracked]
    #[returns(ref)]
    pub variants: Vec<CLikeEnumVariant<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

impl<'db> IntoItemId<'db> for CLikeEnum<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        self.id(db).into()
    }
}

/// A lowered fieldless enum variant and its optional explicit discriminant.
#[salsa::tracked(debug)]
pub struct CLikeEnumVariant<'db> {
    #[returns(ref)]
    pub name: String,
    /// The discriminant value of this variant as a number between [`i128::MIN`]
    /// and [`u128::MAX`], if any.
    #[tracked]
    #[returns(ref)]
    pub discriminant: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

define_item_id!(ModId, ItemKind::Mod, "Identifies a lowered Rust module.");

/// A lowered Rust module.
#[salsa::tracked(debug)]
pub struct Mod<'db> {
    pub id: ModId<'db>,
    /// Items defined directly inside this module.
    #[tracked]
    #[returns(ref)]
    pub items: Vec<Item<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

impl<'db> Mod<'db> {
    pub fn fns(self, db: &'db dyn Db) -> impl Iterator<Item = Fn<'db>> + Clone + 'db {
        self.items(db).iter().filter_map(|item| match item {
            Item::Fn(fn_) => Some(*fn_),
            _ => None,
        })
    }

    pub fn structs(self, db: &'db dyn Db) -> impl Iterator<Item = Struct<'db>> + Clone + 'db {
        self.items(db).iter().filter_map(|item| match item {
            Item::Struct(struct_) => Some(*struct_),
            _ => None,
        })
    }

    pub fn enums(self, db: &'db dyn Db) -> impl Iterator<Item = Enum<'db>> + Clone + 'db {
        self.items(db).iter().filter_map(|item| match item {
            Item::Enum(enum_) => Some(*enum_),
            _ => None,
        })
    }
}

impl<'db> IntoItemId<'db> for Mod<'db> {
    fn into_item_id(self, db: &'db dyn Db) -> ItemId<'db> {
        self.id(db).into()
    }
}
