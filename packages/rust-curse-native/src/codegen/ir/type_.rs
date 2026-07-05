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
    Char,
    Str,
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
    /// Returns this primitive's Rust source name for diagnostics.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::ir::Primitive;
    ///
    /// assert_eq!(Primitive::Str.rs_name(), "str");
    /// ```
    pub fn rs_name(&self) -> &'static str {
        match self {
            Primitive::Boolean => "bool",
            Primitive::Char => "char",
            Primitive::Str => "str",
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

    /// Returns this primitive's JavaScript source name for diagnostics.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::ir::Primitive;
    ///
    /// assert_eq!(Primitive::Boolean.js_name(), "boolean");
    /// ```
    pub fn js_name(&self) -> &'static str {
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
            Primitive::Str => "string",
        }
    }

    /// Returns the generated cast helper function name for this primitive for
    /// diagnostics.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::{db::DbImpl, ir::Primitive};
    ///
    /// let db = DbImpl::default();
    ///
    /// assert_eq!(Primitive::f64.cast_fn_name(&db), "__cast_f64");
    /// ```
    pub fn cast_fn_name(&self) -> &'static str {
        match self {
            Primitive::Boolean => "__cast_bool",
            Primitive::Char => "__cast_char",
            Primitive::Str => "__cast_String",
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

impl Type {
    /// Returns this type's Rust source name for diagnostics.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::ir::{Primitive, Type};
    ///
    /// let type_ = Type::Primitive(Primitive::Str);
    ///
    /// assert_eq!(type_.rs_name(), "String");
    /// ```
    pub fn rs_name(&self) -> &'static str {
        match self {
            Type::Primitive(primitive) => primitive.rs_name(),
        }
    }

    /// Returns this type's JavaScript source name for diagnostics.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::ir::{Primitive, Type};
    ///
    /// let type_ = Type::Primitive(Primitive::Boolean);
    ///
    /// assert_eq!(type_.js_name(), "boolean");
    /// ```
    pub fn js_name(&self) -> &'static str {
        match self {
            Type::Primitive(primitive) => primitive.js_name(),
        }
    }

    /// Returns the generated cast helper function name for this type.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_curse::codegen::{
    ///     db::DbImpl,
    ///     ir::{Primitive, Type},
    /// };
    ///
    /// let db = DbImpl::default();
    /// let type_ = Type::Primitive(Primitive::f64);
    ///
    /// assert_eq!(type_.cast_fn_name(&db), "__cast_f64");
    /// ```
    pub fn cast_fn_name(&self) -> &'static str {
        match self {
            Type::Primitive(primitive) => primitive.cast_fn_name(),
        }
    }
}
