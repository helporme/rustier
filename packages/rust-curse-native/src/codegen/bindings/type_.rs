use crate::codegen::ir;

/// Compiles a type to a string.
pub fn compile_type(type_: &ir::Type) -> &'static str {
    match type_ {
        ir::Type::Primitive(primitive) => compile_primitive(*primitive),
    }
}

/// Compiles a primitive type to a string.
pub fn compile_primitive(primitive: ir::Primitive) -> &'static str {
    match primitive {
        ir::Primitive::Boolean => "bool",
        ir::Primitive::Str => "String",
        ir::Primitive::i8 => "i8",
        ir::Primitive::u8 => "u8",
        ir::Primitive::i16 => "i16",
        ir::Primitive::u16 => "u16",
        ir::Primitive::i32 => "i32",
        ir::Primitive::u32 => "u32",
        ir::Primitive::f64 => "f64",
    }
}
