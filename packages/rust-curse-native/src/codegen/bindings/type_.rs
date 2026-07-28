//! A module that defines types used for binding compilation.

use crate::{
    codegen::{
        cast::DirectCastRule,
        db::Db,
        fragment::FragmentOnce,
        ir::{
            item::{ItemId, ItemPath},
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
    Item(ItemPath<'db>),
    Ref {
        is_mutable: bool,
        inner: Box<SourceType<'db>>,
    },
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
    Generated {
        /// The target item for which a boundary type is generated.
        target: ItemId<'db>,
    },
}

impl<'db> Type<'db> {
    /// Gets the [`BoundaryType`] used to accept this type from the JavaScript.
    pub fn boundary_in(self, _db: &'db dyn Db) -> BoundaryType<'db> {
        match self {
            Type::Primitive(_) | Type::String | Type::Str => BoundaryType::JsAny,
            Type::Struct(id) => BoundaryType::Generated { target: id.into() },
            Type::Enum(id) => BoundaryType::Generated { target: id.into() },
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
            Type::Struct(id) => BoundaryType::Generated { target: id.into() },
            Type::Enum(id) => BoundaryType::Generated { target: id.into() },
            Type::Str => unreachable!("str cannot be returned"),
        }
    }
}

/// A rule to cast an owned source type to a reference source type.
pub struct CastOwnedTypeToRefTypeRule;

impl<'db> DirectCastRule<'db, SourceType<'db>, SourceType<'db>> for CastOwnedTypeToRefTypeRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        match to {
            SourceType::Ref {
                is_mutable: false,
                inner,
                ..
            } => Some(*inner),
            _ => None,
        }
    }

    fn compile(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: SourceType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
        frag!(&{ in_expr })
    }
}

/// A rule to cast an owned source type to a mutable source type.
pub struct CastOwnedTypeToMutTypeRule;

impl<'db> DirectCastRule<'db, SourceType<'db>, SourceType<'db>> for CastOwnedTypeToMutTypeRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        match to {
            SourceType::Ref {
                is_mutable: true,
                inner,
                ..
            } => Some(*inner),
            _ => None,
        }
    }

    fn compile(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: SourceType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
        frag!(&mut { in_expr })
    }
}
