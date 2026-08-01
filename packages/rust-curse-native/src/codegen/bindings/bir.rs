//! Binding-specific intermediate representations for generated Rust source and
//! JavaScript external types.

use crate::{
    codegen::{
        api::Api,
        db::Db,
        fragment::{FragmentOnce, fragment},
        ir::{
            item::ItemId,
            type_::{IrPrimitive, Type},
        },
    },
    frag,
};

/// A type that is accepted by the source rust code.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum SourceType<'db> {
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
        inner: Box<SourceType<'db>>,
    },
}

impl<'db> SourceType<'db> {
    /// Converts this source type into a shared reference type.
    pub fn into_ref(self) -> SourceType<'db> {
        SourceType::Ref {
            is_mutable: false,
            inner: Box::new(self),
        }
    }

    /// Converts this source type into a mutable reference type.
    pub fn into_mut(self) -> SourceType<'db> {
        SourceType::Ref {
            is_mutable: true,
            inner: Box::new(self),
        }
    }

    /// Renders this source type as a fully qualified Rust type declaration.
    pub fn fqn_decl(&self, db: &dyn Db) -> impl FragmentOnce {
        fragment(move |p| match self {
            SourceType::bool => "bool".apply(p),
            SourceType::i8 => "i8".apply(p),
            SourceType::u8 => "u8".apply(p),
            SourceType::i16 => "i16".apply(p),
            SourceType::u16 => "u16".apply(p),
            SourceType::i32 => "i32".apply(p),
            SourceType::u32 => "u32".apply(p),
            SourceType::f64 => "f64".apply(p),
            SourceType::f32 => "f32".apply(p),
            SourceType::char => "char".apply(p),
            SourceType::Str => "str".apply(p),
            SourceType::String => "String".apply(p),
            SourceType::Item { target } => target.fqn(db).apply(p),
            SourceType::Ref { is_mutable, inner } => {
                let prefix = if *is_mutable { "&mut " } else { "&" };
                frag!({prefix}{inner.fqn_decl(db)}).apply(p)
            }
        })
    }

    /// Renders this source type as a mangled identifier fragment.
    pub fn mangled_name(&self, db: &dyn Db) -> impl FragmentOnce {
        fragment(|p| match self {
            SourceType::bool => "_bool".apply(p),
            SourceType::char => "_char".apply(p),
            SourceType::i8 => "_i8".apply(p),
            SourceType::u8 => "_u8".apply(p),
            SourceType::i16 => "_i16".apply(p),
            SourceType::u16 => "_u16".apply(p),
            SourceType::i32 => "_i32".apply(p),
            SourceType::u32 => "_u32".apply(p),
            SourceType::f64 => "_f64".apply(p),
            SourceType::f32 => "_f32".apply(p),
            SourceType::Str => "_str".apply(p),
            SourceType::String => "_String".apply(p),
            SourceType::Item { target } => target.mangled_fqn(db).apply(p),
            SourceType::Ref { is_mutable, inner } => {
                let suffix = if *is_mutable { "_mut" } else { "_ref" };
                frag!({suffix}__{inner.mangled_name(db)}).apply(p)
            }
        })
    }

    /// Returns whether an owned value of this type should be borrowed with a
    /// simple `&expr` or `&mut expr` cast.
    ///
    /// False is usually returned for types that wish to implement borrowing in
    /// some other way, say struct or enum.
    pub fn allows_generic_borrowing_cast(&self) -> bool {
        match self {
            SourceType::bool
            | SourceType::char
            | SourceType::i8
            | SourceType::u8
            | SourceType::i16
            | SourceType::u16
            | SourceType::i32
            | SourceType::u32
            | SourceType::f64
            | SourceType::f32
            | SourceType::Str
            | SourceType::String
            | SourceType::Ref { .. } // technically it's unused since double refs are not supported by ir
            => true,
            SourceType::Item { .. } => false,
        }
    }
}

/// Represents a Rust type exposed across the JavaScript binding boundary.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum ExternType<'db> {
    bool,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    f64,
    String,
    /// Represents the Rust boundary type configured by [`Api::js_type`] for
    /// arbitrary JavaScript values.
    JsAny,
    /// Represents a generated boundary item used to transfer values across the
    /// JavaScript boundary.
    Item {
        /// Identifies the source item represented by the generated boundary
        /// item.
        target: ItemId<'db>,
    },
    Ref(Box<ExternType<'db>>),
}

impl<'db> ExternType<'db> {
    /// Renders this external type as a fully qualified Rust type declaration.
    pub fn fqn_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            ExternType::bool => "bool".apply(p),
            ExternType::i8 => "i8".apply(p),
            ExternType::u8 => "u8".apply(p),
            ExternType::i16 => "i16".apply(p),
            ExternType::u16 => "u16".apply(p),
            ExternType::i32 => "i32".apply(p),
            ExternType::u32 => "u32".apply(p),
            ExternType::f64 => "f64".apply(p),
            ExternType::String => "String".apply(p),
            ExternType::JsAny => Api::get(db).js_type(db).apply(p),
            ExternType::Item { target } => target.extern_name(db).apply(p),
            ExternType::Ref(inner) => frag!(&{ inner.fqn_decl(db) }).apply(p),
        })
    }
}

impl<'db> Type<'db> {
    /// Returns the external type used to receive a JavaScript value for this
    /// type.
    pub(super) fn extern_in(self) -> ExternType<'db> {
        match self {
            Type::Primitive(_) | Type::String | Type::Str => ExternType::JsAny,
            Type::Struct(id) => ExternType::Ref(Box::new(ExternType::Item { target: id.into() })),
            Type::Enum(id) => ExternType::Ref(Box::new(ExternType::Item { target: id.into() })),
        }
    }

    /// Returns the external type used to return this type to JavaScript.
    pub(super) fn extern_out(self) -> ExternType<'db> {
        match self {
            Type::Primitive(IrPrimitive::Boolean) => ExternType::bool,
            Type::Primitive(IrPrimitive::i8) => ExternType::i8,
            Type::Primitive(IrPrimitive::u8) => ExternType::u8,
            Type::Primitive(IrPrimitive::i16) => ExternType::i16,
            Type::Primitive(IrPrimitive::u16) => ExternType::u16,
            Type::Primitive(IrPrimitive::i32) => ExternType::i32,
            Type::Primitive(IrPrimitive::u32) | Type::Primitive(IrPrimitive::Char) => {
                ExternType::u32
            }
            Type::Primitive(IrPrimitive::f64) | Type::Primitive(IrPrimitive::f32) => {
                ExternType::f64
            }
            Type::String => ExternType::String,
            Type::Struct(id) => ExternType::Item { target: id.into() },
            Type::Enum(id) => ExternType::Item { target: id.into() },
            Type::Str => unreachable!("str cannot be returned"),
        }
    }

    /// Returns the owned source type corresponding to this IR type.
    pub(super) fn source_type_owned(self) -> SourceType<'db> {
        match self {
            Type::Primitive(IrPrimitive::Boolean) => SourceType::bool,
            Type::Primitive(IrPrimitive::Char) => SourceType::char,
            Type::Primitive(IrPrimitive::i8) => SourceType::i8,
            Type::Primitive(IrPrimitive::u8) => SourceType::u8,
            Type::Primitive(IrPrimitive::i16) => SourceType::i16,
            Type::Primitive(IrPrimitive::u16) => SourceType::u16,
            Type::Primitive(IrPrimitive::i32) => SourceType::i32,
            Type::Primitive(IrPrimitive::u32) => SourceType::u32,
            Type::Primitive(IrPrimitive::f32) => SourceType::f32,
            Type::Primitive(IrPrimitive::f64) => SourceType::f64,
            Type::Str => SourceType::Str,
            Type::String => SourceType::String,
            Type::Struct(struct_id) => SourceType::Item {
                target: struct_id.into(),
            },
            Type::Enum(enum_id) => SourceType::Item {
                target: enum_id.into(),
            },
        }
    }

    /// Returns whether this type is compiled through an
    /// [`ExternClass`](super::extern_class::ExternClass) wrapper.
    pub(super) fn uses_class(self) -> bool {
        match self {
            Type::Struct(_) | Type::Enum(_) => true,
            Type::Primitive(_) | Type::Str | Type::String => false,
        }
    }

    /// Returns whether this type is compiled through a
    /// [`TetheringStruct`](super::tethering::TetheringStruct).
    pub(super) fn uses_tethering(self) -> bool {
        // An ExternClass wraps a tethering struct, so class-backed types always use
        // tethering.
        self.uses_class()
    }
}
