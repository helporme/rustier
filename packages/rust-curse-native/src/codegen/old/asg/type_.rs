use crate::codegen::asg::{EnumDef, StructDef};

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub enum Primitive {
    Boolean,
    Char,
    Str,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    i64,
    u64,
    isize,
    usize,
    f32,
    f64,
}

/// Represents a type in the ASG.
#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub enum Type<'db> {
    Primitive(Primitive),
    Struct(StructDef<'db>),
    Enum(EnumDef<'db>),
    Ref {
        is_mutable: bool,
        ty: Box<Type<'db>>,
    },
}
