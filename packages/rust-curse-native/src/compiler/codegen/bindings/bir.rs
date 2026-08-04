//! Binding-specific intermediate representations for generated Rust source and
//! JavaScript external types.

use crate::{
    compiler::{
        codegen::api::Api,
        codegen::fragment::{FragmentOnce, fragment},
        db::Db,
        ir::{ItemId, Type, ValueType},
    },
    frag,
};

impl<'db> Type<'db> {
    /// Converts this source type into a shared reference type.
    pub(super) fn into_ref(self) -> Type<'db> {
        Type::Borrowed {
            is_mutable: false,
            inner: self.deref(),
        }
    }

    /// Converts this source type into a mutable reference type.
    pub(super) fn into_mut(self) -> Type<'db> {
        Type::Borrowed {
            is_mutable: true,
            inner: self.deref(),
        }
    }

    /// Renders this source type as a fully qualified Rust type declaration.
    pub(super) fn fqn_decl(&self, db: &dyn Db) -> impl FragmentOnce {
        fragment(move |p| {
            let (prefix, value) = match self {
                Type::Value(value) => ("", value),
                Type::Borrowed { is_mutable, inner } => {
                    (if *is_mutable { "&mut " } else { "&" }, inner)
                }
            };

            prefix.apply(p);
            match value {
                ValueType::bool => "bool".apply(p),
                ValueType::i8 => "i8".apply(p),
                ValueType::u8 => "u8".apply(p),
                ValueType::i16 => "i16".apply(p),
                ValueType::u16 => "u16".apply(p),
                ValueType::i32 => "i32".apply(p),
                ValueType::u32 => "u32".apply(p),
                ValueType::f64 => "f64".apply(p),
                ValueType::f32 => "f32".apply(p),
                ValueType::char => "char".apply(p),
                ValueType::Str => "str".apply(p),
                ValueType::String => "String".apply(p),
                ValueType::Struct(id) => ItemId::from(*id).fqn(db).apply(p),
                ValueType::Enum(id) => ItemId::from(*id).fqn(db).apply(p),
            }
        })
    }

    /// Renders this source type as a mangled identifier fragment.
    pub(super) fn mangled_name(&self, db: &dyn Db) -> impl FragmentOnce {
        fragment(|p| {
            let (prefix, value) = match self {
                Type::Value(value) => ("", value),
                Type::Borrowed { is_mutable, inner } => {
                    (if *is_mutable { "_mut__" } else { "_ref__" }, inner)
                }
            };

            prefix.apply(p);
            match value {
                ValueType::bool => "_bool".apply(p),
                ValueType::char => "_char".apply(p),
                ValueType::i8 => "_i8".apply(p),
                ValueType::u8 => "_u8".apply(p),
                ValueType::i16 => "_i16".apply(p),
                ValueType::u16 => "_u16".apply(p),
                ValueType::i32 => "_i32".apply(p),
                ValueType::u32 => "_u32".apply(p),
                ValueType::f64 => "_f64".apply(p),
                ValueType::f32 => "_f32".apply(p),
                ValueType::Str => "_str".apply(p),
                ValueType::String => "_String".apply(p),
                ValueType::Struct(id) => ItemId::from(*id).mangled_fqn(db).apply(p),
                ValueType::Enum(id) => ItemId::from(*id).mangled_fqn(db).apply(p),
            }
        })
    }

    /// Returns whether an owned value of this type supports a simple borrowing
    /// cast.
    pub(super) fn allows_generic_borrowing_cast(&self) -> bool {
        match self.deref() {
            ValueType::bool
            | ValueType::char
            | ValueType::i8
            | ValueType::u8
            | ValueType::i16
            | ValueType::u16
            | ValueType::i32
            | ValueType::u32
            | ValueType::f64
            | ValueType::f32
            | ValueType::Str
            | ValueType::String => true,
            ValueType::Struct(_) | ValueType::Enum(_) => false,
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

impl<'db> ValueType<'db> {
    /// Returns the external type used to receive a JavaScript value for this
    /// type.
    pub(super) fn extern_in(self) -> ExternType<'db> {
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
            | ValueType::String
            | ValueType::Str => ExternType::JsAny,
            ValueType::Struct(id) => {
                ExternType::Ref(Box::new(ExternType::Item { target: id.into() }))
            }
            ValueType::Enum(id) => {
                ExternType::Ref(Box::new(ExternType::Item { target: id.into() }))
            }
        }
    }

    /// Returns the external type used to return this type to JavaScript.
    pub(super) fn extern_out(self) -> ExternType<'db> {
        match self {
            ValueType::bool => ExternType::bool,
            ValueType::i8 => ExternType::i8,
            ValueType::u8 => ExternType::u8,
            ValueType::i16 => ExternType::i16,
            ValueType::u16 => ExternType::u16,
            ValueType::i32 => ExternType::i32,
            ValueType::u32 | ValueType::char => ExternType::u32,
            ValueType::f64 | ValueType::f32 => ExternType::f64,
            ValueType::String => ExternType::String,
            ValueType::Struct(id) => ExternType::Item { target: id.into() },
            ValueType::Enum(id) => ExternType::Item { target: id.into() },
            ValueType::Str => unreachable!("str cannot be returned"),
        }
    }

    /// Returns whether this type is compiled through an
    /// [`ExternClass`](super::extern_class::ExternClass) wrapper.
    pub(super) fn uses_class(self) -> bool {
        match self {
            ValueType::Struct(_) | ValueType::Enum(_) => true,
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
            | ValueType::String => false,
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
