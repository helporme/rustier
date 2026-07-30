//! A module that defines types used for binding compilation.

use crate::{
    codegen::{
        cast::DirectCastRule,
        db::Db,
        fragment::FragmentOnce,
        ir::{
            item::ItemId,
            type_::{Primitive, Type},
        },
    },
    frag,
};

/// A type that is accepted by source code.
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
    /// Whether this type supports automatic casting from owned type to
    /// reference type using simple `&expr` or `&mut expr` borrowing
    /// expressions.
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

/// A type that is passed across the boundary using active [`Api`] bindings.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum BoundaryType<'db> {
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
}

impl<'db> Type<'db> {
    /// Gets the [`BoundaryType`] used to accept this type from the JavaScript.
    pub fn boundary_in(self, _db: &'db dyn Db) -> BoundaryType<'db> {
        match self {
            Type::Primitive(_) | Type::String | Type::Str => BoundaryType::JsAny,
            Type::Struct(id) => BoundaryType::Item { target: id.into() },
            Type::Enum(id) => BoundaryType::Item { target: id.into() },
        }
    }

    /// Gets the [`BoundaryType`] used to return this type to the JavaScript.
    pub fn boundary_out(self, _db: &'db dyn Db) -> BoundaryType<'db> {
        match self {
            Type::Primitive(Primitive::Boolean) => BoundaryType::bool,
            Type::Primitive(Primitive::i8) => BoundaryType::i8,
            Type::Primitive(Primitive::u8) => BoundaryType::u8,
            Type::Primitive(Primitive::i16) => BoundaryType::i16,
            Type::Primitive(Primitive::u16) => BoundaryType::u16,
            Type::Primitive(Primitive::i32) => BoundaryType::i32,
            Type::Primitive(Primitive::u32) | Type::Primitive(Primitive::Char) => BoundaryType::u32,
            Type::Primitive(Primitive::f64) | Type::Primitive(Primitive::f32) => BoundaryType::f64,
            Type::String => BoundaryType::String,
            Type::Struct(id) => BoundaryType::Item { target: id.into() },
            Type::Enum(id) => BoundaryType::Item { target: id.into() },
            Type::Str => unreachable!("str cannot be returned"),
        }
    }
}

/// A rule to cast an owned source type to a reference source type.
pub struct CastOwnedTypeToBorrowedTypeRule;

impl<'db> DirectCastRule<'db, SourceType<'db>, SourceType<'db>>
    for CastOwnedTypeToBorrowedTypeRule
{
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        match to {
            SourceType::Ref { inner, .. } if inner.allows_generic_borrowing_cast() => Some(*inner),
            _ => None,
        }
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        to: SourceType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let SourceType::Ref { is_mutable, .. } = to else {
            unreachable!()
        };

        if is_mutable {
            frag!(&mut { in_expr })
        } else {
            frag!(&{ in_expr })
        }
    }
}
