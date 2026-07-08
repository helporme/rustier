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
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::ir::type_::Primitive;
    ///
    /// assert_eq!(Primitive::i8.rs_name(), "i8");
    /// ```
    pub fn rs_name(&self) -> &'static str {
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
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::ir::type_::Primitive;
    ///
    /// assert_eq!(Primitive::Boolean.ts_name(), "boolean");
    /// ```
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
    ///
    /// See [`crate::codegen::bindings::cast`].
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::ir::type_::Primitive;
    ///
    /// assert_eq!(Primitive::f64.cast_fn_name(), "__cast_f64");
    /// ```
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

/// A type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Type {
    Primitive(Primitive),
    Str,
    String,
}

impl Type {
    /// Returns this type's Rust name used in source code.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::ir::type_::Type;
    ///
    /// let type_ = Type::String;
    ///
    /// assert_eq!(type_.rs_name(), "String");
    /// ```
    pub fn rs_name(&self) -> &'static str {
        match self {
            Type::Primitive(primitive) => primitive.rs_name(),
            Type::Str => "str",
            Type::String => "String",
        }
    }

    /// Returns this type's TypeScript name.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::ir::type_::{Primitive, Type};
    ///
    /// let type_ = Type::Primitive(Primitive::Boolean);
    ///
    /// assert_eq!(type_.ts_name(), "boolean");
    /// ```
    pub fn ts_name(&self) -> &'static str {
        match self {
            Type::Primitive(primitive) => primitive.ts_name(),
            Type::Str | Type::String => "string",
        }
    }

    /// Returns the cast function name for this type.
    ///
    /// See [`crate::codegen::bindings::cast`].
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::ir::type_::{Primitive, Type};
    ///
    /// let type_ = Type::Primitive(Primitive::f64);
    ///
    /// assert_eq!(type_.cast_fn_name(), "__cast_f64");
    /// ```
    pub fn cast_fn_name(&self) -> &'static str {
        match self {
            Type::Primitive(primitive) => primitive.cast_fn_name(),
            Type::Str | Type::String => "__cast_String",
        }
    }
}
