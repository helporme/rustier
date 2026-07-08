use crate::codegen::ir::type_::{Primitive, Type};

/// Compiles a type to a string.
pub fn compile_type(type_: &Type) -> &'static str {
    match type_ {
        Type::Primitive(primitive) => compile_primitive(*primitive),
        Type::Str | Type::String => "String",
    }
}

/// Compiles a primitive type to a string.
pub fn compile_primitive(primitive: Primitive) -> &'static str {
    match primitive {
        Primitive::Boolean => "bool",
        Primitive::i8 => "i8",
        Primitive::u8 => "u8",
        Primitive::i16 => "i16",
        Primitive::u16 => "u16",
        Primitive::i32 => "i32",
        Primitive::u32 | Primitive::Char => "u32",
        Primitive::f64 | Primitive::f32 => "f64",
    }
}
