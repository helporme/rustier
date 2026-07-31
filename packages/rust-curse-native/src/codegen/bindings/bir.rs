//! IR types used to support bindings generation.

use crate::{
    codegen::{
        api::Api,
        db::Db,
        fragment::{FragmentOnce, fragment},
        ir::{
            item::{ItemId, ItemKind},
            type_::{IrPrimitive, IrType},
        },
    },
    frag,
};

/// A type that is accepted by the source code.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum BirSourceType<'db> {
    bool,
    char,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    f64,
    f32,
    Str,
    String,
    Item {
        target: ItemId<'db>,
    },
    Ref {
        is_mutable: bool,
        inner: Box<BirSourceType<'db>>,
    },
}

impl<'db> BirSourceType<'db> {
    pub fn fqn_decl(&self, db: &dyn Db) -> impl FragmentOnce {
        fragment(move |p| match self {
            BirSourceType::bool => "bool".apply(p),
            BirSourceType::i8 => "i8".apply(p),
            BirSourceType::u8 => "u8".apply(p),
            BirSourceType::i16 => "i16".apply(p),
            BirSourceType::u16 => "u16".apply(p),
            BirSourceType::i32 => "i32".apply(p),
            BirSourceType::u32 => "u32".apply(p),
            BirSourceType::f64 => "f64".apply(p),
            BirSourceType::f32 => "f32".apply(p),
            BirSourceType::char => "char".apply(p),
            BirSourceType::Str => "str".apply(p),
            BirSourceType::String => "String".apply(p),
            BirSourceType::Item { target } => target.fqn(db).apply(p),
            BirSourceType::Ref { is_mutable, inner } => {
                let prefix = if *is_mutable { "&mut " } else { "&" };
                frag!({prefix}{inner.fqn_decl(db)}).apply(p)
            }
        })
    }

    /// A mangled name of the source type.
    pub fn mangled_name(&self, db: &dyn Db) -> impl FragmentOnce {
        fragment(|p| match self {
            BirSourceType::bool => "_bool".apply(p),
            BirSourceType::char => "_char".apply(p),
            BirSourceType::i8 => "_i8".apply(p),
            BirSourceType::u8 => "_u8".apply(p),
            BirSourceType::i16 => "_i16".apply(p),
            BirSourceType::u16 => "_u16".apply(p),
            BirSourceType::i32 => "_i32".apply(p),
            BirSourceType::u32 => "_u32".apply(p),
            BirSourceType::f64 => "_f64".apply(p),
            BirSourceType::f32 => "_f32".apply(p),
            BirSourceType::Str => "_str".apply(p),
            BirSourceType::String => "_String".apply(p),
            BirSourceType::Item { target } => target.mangled_fqn(db).apply(p),
            BirSourceType::Ref { is_mutable, inner } => {
                let suffix = if *is_mutable { "_mut" } else { "_ref" };
                frag!({suffix}__{inner.mangled_name(db)}).apply(p)
            }
        })
    }

    /// Whether this type supports automatic casting from owned type to
    /// reference type using simple `&expr` or `&mut expr` borrowing
    /// expressions.
    pub fn allows_generic_borrowing_cast(&self) -> bool {
        match self {
            BirSourceType::bool
            | BirSourceType::char
            | BirSourceType::i8
            | BirSourceType::u8
            | BirSourceType::i16
            | BirSourceType::u16
            | BirSourceType::i32
            | BirSourceType::u32
            | BirSourceType::f64
            | BirSourceType::f32
            | BirSourceType::Str
            | BirSourceType::String
            | BirSourceType::Ref { .. } // technically it's unused since double refs are not supported by ir
            => true,
            BirSourceType::Item { .. } => false,
        }
    }

    /// Whether this type is using
    /// [`TetheringStruct`](super::tethering::TetheringStruct) for it's
    /// runtime representation.
    pub fn is_tethered(&self, db: &dyn Db) -> bool {
        match self {
            BirSourceType::Item { target } => match target.kind(db) {
                ItemKind::Struct | ItemKind::Enum => true,
                ItemKind::CLikeEnum | ItemKind::Mod | ItemKind::Fn => false,
            },
            BirSourceType::bool
            | BirSourceType::char
            | BirSourceType::i8
            | BirSourceType::u8
            | BirSourceType::i16
            | BirSourceType::u16
            | BirSourceType::i32
            | BirSourceType::u32
            | BirSourceType::f64
            | BirSourceType::f32
            | BirSourceType::Str
            | BirSourceType::String
            | BirSourceType::Ref { .. } => false,
        }
    }
}

/// A type that is exposed to the JavaScript.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum BirBoundaryType<'db> {
    bool,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    f64,
    f32,
    String,
    /// JavaScript "any" type. Actual runtime type is specified by
    /// [`crate::codegen::api::Api::js_type`]
    JsAny,
    /// Generated item (like struct) used exclusively to pass values across the
    /// boundary.
    Item {
        /// The target item for which a boundary type is generated.
        target: ItemId<'db>,
    },
    Ref(Box<BirBoundaryType<'db>>),
}

impl<'db> BirBoundaryType<'db> {
    /// Redners the type as type declaration syntax.
    pub fn fqn_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            BirBoundaryType::bool => "bool".apply(p),
            BirBoundaryType::i8 => "i8".apply(p),
            BirBoundaryType::u8 => "u8".apply(p),
            BirBoundaryType::i16 => "i16".apply(p),
            BirBoundaryType::u16 => "u16".apply(p),
            BirBoundaryType::i32 => "i32".apply(p),
            BirBoundaryType::u32 => "u32".apply(p),
            BirBoundaryType::f64 => "f64".apply(p),
            BirBoundaryType::f32 => "f32".apply(p),
            BirBoundaryType::String => "String".apply(p),
            BirBoundaryType::JsAny => Api::get(db).js_type(db).apply(p),
            BirBoundaryType::Item { target } => target.extern_name(db).apply(p),
            BirBoundaryType::Ref(inner) => frag!(&{ inner.fqn_decl(db) }).apply(p),
        })
    }
}

impl<'db> IrType<'db> {
    /// Gets the [`BirBoundaryType`] used to accept this type from the
    /// JavaScript.
    pub fn boundary_in(self) -> BirBoundaryType<'db> {
        match self {
            IrType::Primitive(_) | IrType::String | IrType::Str => BirBoundaryType::JsAny,
            IrType::Struct(id) => {
                BirBoundaryType::Ref(Box::new(BirBoundaryType::Item { target: id.into() }))
            }
            IrType::Enum(id) => {
                BirBoundaryType::Ref(Box::new(BirBoundaryType::Item { target: id.into() }))
            }
        }
    }

    /// Gets the [`BirBoundaryType`] used to return this type to the JavaScript.
    pub fn boundary_out(self) -> BirBoundaryType<'db> {
        match self {
            IrType::Primitive(IrPrimitive::Boolean) => BirBoundaryType::bool,
            IrType::Primitive(IrPrimitive::i8) => BirBoundaryType::i8,
            IrType::Primitive(IrPrimitive::u8) => BirBoundaryType::u8,
            IrType::Primitive(IrPrimitive::i16) => BirBoundaryType::i16,
            IrType::Primitive(IrPrimitive::u16) => BirBoundaryType::u16,
            IrType::Primitive(IrPrimitive::i32) => BirBoundaryType::i32,
            IrType::Primitive(IrPrimitive::u32) | IrType::Primitive(IrPrimitive::Char) => {
                BirBoundaryType::u32
            }
            IrType::Primitive(IrPrimitive::f64) | IrType::Primitive(IrPrimitive::f32) => {
                BirBoundaryType::f64
            }
            IrType::String => BirBoundaryType::String,
            IrType::Struct(id) => BirBoundaryType::Item { target: id.into() },
            IrType::Enum(id) => BirBoundaryType::Item { target: id.into() },
            IrType::Str => unreachable!("str cannot be returned"),
        }
    }

    /// Returns the owned source type of the ir type.
    pub fn source_type_owned(self) -> BirSourceType<'db> {
        match self {
            IrType::Primitive(IrPrimitive::Boolean) => BirSourceType::bool,
            IrType::Primitive(IrPrimitive::Char) => BirSourceType::char,
            IrType::Primitive(IrPrimitive::i8) => BirSourceType::i8,
            IrType::Primitive(IrPrimitive::u8) => BirSourceType::u8,
            IrType::Primitive(IrPrimitive::i16) => BirSourceType::i16,
            IrType::Primitive(IrPrimitive::u16) => BirSourceType::u16,
            IrType::Primitive(IrPrimitive::i32) => BirSourceType::i32,
            IrType::Primitive(IrPrimitive::u32) => BirSourceType::u32,
            IrType::Primitive(IrPrimitive::f32) => BirSourceType::f32,
            IrType::Primitive(IrPrimitive::f64) => BirSourceType::f64,
            IrType::Str => BirSourceType::Str,
            IrType::String => BirSourceType::String,
            IrType::Struct(struct_id) => BirSourceType::Item {
                target: struct_id.into(),
            },
            IrType::Enum(enum_id) => BirSourceType::Item {
                target: enum_id.into(),
            },
        }
    }
}
