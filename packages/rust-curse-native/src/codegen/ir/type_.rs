use crate::codegen::{
    db::Db,
    ir::{enum_::EnumId, struct_::StructId},
};

/// A primitive type.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, salsa::Update)]
pub enum Primitive {
    Boolean,
    Char,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    f32,
    f64,
}

impl Primitive {
    /// Returns this primitive's Rust name.
    pub fn fqn(&self) -> &'static str {
        match self {
            Primitive::Boolean => "bool",
            Primitive::Char => "char",
            Primitive::i8 => "i8",
            Primitive::u8 => "u8",
            Primitive::i16 => "i16",
            Primitive::u16 => "u16",
            Primitive::i32 => "i32",
            Primitive::u32 => "u32",
            Primitive::f32 => "f32",
            Primitive::f64 => "f64",
        }
    }

    /// Returns this primitive's TypeScript name.
    pub fn ts_name(&self) -> &'static str {
        match self {
            Primitive::Boolean => "boolean",
            Primitive::Char
            | Primitive::i8
            | Primitive::u8
            | Primitive::i16
            | Primitive::u16
            | Primitive::i32
            | Primitive::u32
            | Primitive::f32
            | Primitive::f64 => "number",
        }
    }

    /// Returns the cast function name for this primitive.
    pub fn cast_fn_name(&self) -> &'static str {
        match self {
            Primitive::Boolean => "__cast_bool",
            Primitive::Char => "__cast_char",
            Primitive::i8 => "__cast_i8",
            Primitive::u8 => "__cast_u8",
            Primitive::i16 => "__cast_i16",
            Primitive::u16 => "__cast_u16",
            Primitive::i32 => "__cast_i32",
            Primitive::u32 => "__cast_u32",
            Primitive::f32 => "__cast_f32",
            Primitive::f64 => "__cast_f64",
        }
    }
}

// note: adding new variants here means that type must be properly handled in
// all parts of the codebase _at once_. It's effectively impossible to add
// step-by-step support for some new variant without todo!() panics which
// disable tests. Maybe it's a good thing, but I am not entirely sure.

/// A type as represented by rust-curse.
///
/// Each variant describes a Rust type category that rust-curse handles
/// differently during binding generation. This is not a full Rust type model
/// like [rustdoc_types::Type], but every value can be mapped back to its
/// source Rust type.
///
/// For example, String has its own variant even though it is a struct,
/// because rust-curse maps it directly to JavaScript string, while other
/// structs are mapped to generated classes.
///
/// It also means that there is no "ref" or "mut ref" variant, since rust-curse
/// doesn't really support refs in all cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Type<'db> {
    Primitive(Primitive),
    Str,
    String,
    Struct(StructId<'db>),
    Enum(EnumId<'db>),
}

impl<'db> Type<'db> {
    /// Returns this type's Rust name used in source code.
    pub fn fqn(&self, db: &'db dyn Db) -> &'db str {
        match self {
            Type::Primitive(primitive) => primitive.fqn(),
            Type::Str => "str",
            Type::String => "String",
            Type::Struct(id) => id.fqn(db),
            Type::Enum(id) => id.fqn(db),
        }
    }

    /// Returns this type's TypeScript name.
    pub fn ts_name(&self, db: &'db dyn Db) -> String {
        match self {
            Type::Primitive(primitive) => primitive.ts_name().to_owned(),
            Type::Str | Type::String => "string".to_owned(),
            Type::Struct(id) => id.name(db).to_owned(),
            Type::Enum(id) => id.name(db).to_owned(),
        }
    }

    /// Returns the cast function name for this type.
    // todo: function should be moved to the cast.rs module and removed from here
    pub fn cast_fn_name(&self, db: &'db dyn Db) -> String {
        match self {
            Type::Primitive(primitive) => primitive.cast_fn_name().to_owned(),
            Type::Str | Type::String => "__cast_String".to_owned(),
            // todo: never produced actaully
            Type::Struct(id) => format!("__cast_{}", id.binding_name(db)),
            Type::Enum(id) => format!("__cast_{}", id.binding_name(db)),
        }
    }
}
