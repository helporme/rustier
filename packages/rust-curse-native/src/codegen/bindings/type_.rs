//! A module that defines types used for binding compilation.

use crate::{
    codegen::{
        cast::DirectCastRule,
        db::Db,
        fragment::FragmentOnce,
        ir::item::ItemId,
        ir::type_::{Primitive, Type},
    },
    frag,
};

/// A type that is accepted by source code.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct SourceType<'db> {
    /// Inner type that represent a source type.
    pub value: Type<'db>,
    /// Access mode used to pass the value.
    pub mode: PassMode,
}

/// Specifies a way to pass a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum PassMode {
    Ref,
    Mut,
    Owned,
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
    /// Path to the type for which a generated boundary type is available.
    Generated(ItemId<'db>),
    /// JavaScript "any" type. Actual runtime type is specified by
    /// [`crate::codegen::api::Api::js_type`]
    JsAny,
}

/// A rule to cast an owned source type to a reference source type.
pub struct CastOwnedTypeToRefTypeRule;

impl<'db> DirectCastRule<'db, SourceType<'db>, SourceType<'db>> for CastOwnedTypeToRefTypeRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        if to.mode == PassMode::Ref {
            Some(SourceType {
                value: to.value,
                mode: PassMode::Owned,
            })
        } else {
            None
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
        if to.mode == PassMode::Mut {
            Some(SourceType {
                value: to.value,
                mode: PassMode::Owned,
            })
        } else {
            None
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

impl<'db> Type<'db> {
    /// Gets the type into a [source type][SourceType] with the default
    /// [`PassMode::Owned`].
    pub fn source_type(self) -> SourceType<'db> {
        SourceType {
            value: self,
            // by default all types are passed as owned values
            mode: PassMode::Owned,
        }
    }

    /// Gets the fully qualified name of the type used as a boundary-in type.
    pub fn boundary_in(self, _db: &'db dyn Db) -> BoundaryType<'db> {
        match self {
            Type::Primitive(_) | Type::String | Type::Str => BoundaryType::JsAny,
            Type::Struct(id) => BoundaryType::Generated(id.into()),
            Type::Enum(id) => BoundaryType::Generated(id.into()),
        }
    }

    /// Gets the fully qualified name of the type used as a boundary-out type.
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
            Type::Struct(id) => BoundaryType::Generated(id.into()),
            Type::Enum(id) => BoundaryType::Generated(id.into()),
            Type::Str => unreachable!("str cannot be returned"),
        }
    }
}
