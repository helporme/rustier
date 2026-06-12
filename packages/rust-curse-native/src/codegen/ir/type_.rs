/// A type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Type {
    Primitive(Primitive),
}

/// A primitive type.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, salsa::Update)]
pub enum Primitive {
    Boolean,
    Str,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    f64,
}
