use crate::{
    compiler::{
        codegen::fragment::{FragmentOnce, Printer, frag, fragment},
        db::Db,
        ir::{
            rust::{AsItem, FieldName, ValueType},
            shim::ShimType,
        },
    },
};

impl<'db> ShimType<'db> {
    pub(super) fn render_type_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            ShimType::f64 => "f64".apply(p),
            ShimType::String => "String".apply(p),
            ShimType::bool => "bool".apply(p),
            ShimType::Class {
                is_borrowed: true,
                inner,
            } => frag!(&{ inner.symbol(db) }).apply(p),
            ShimType::Class {
                is_borrowed: false,
                inner,
            } => inner.symbol(db).apply(p),
        })
    }

    pub(super) fn uses_tethering(self) -> bool {
        matches!(self, ShimType::Class { .. })
    }
}

impl FragmentOnce for &FieldName {
    fn apply(self, p: &mut Printer) {
        match self {
            FieldName::Idx(idx) => idx.apply(p),
            FieldName::Name(name) => name.apply(p),
        }
    }
}

impl<'db> ValueType<'db> {
    pub(super) fn render_fqn(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            ValueType::bool => "bool".apply(p),
            ValueType::i8 => "i8".apply(p),
            ValueType::u8 => "u8".apply(p),
            ValueType::i16 => "i16".apply(p),
            ValueType::u16 => "u16".apply(p),
            ValueType::i32 => "i32".apply(p),
            ValueType::u32 => "u32".apply(p),
            ValueType::f64 => "f64".apply(p),
            ValueType::String => "String".apply(p),
            ValueType::char => "char".apply(p),
            ValueType::f32 => "f32".apply(p),
            ValueType::Str => "str".apply(p),
            ValueType::Struct(struct_) => struct_.fqn(db).apply(p),
            ValueType::Enum(enum_) => enum_.fqn(db).apply(p),
        })
    }

    pub(super) fn render_mangled_name(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            ValueType::bool => "__bool".apply(p),
            ValueType::i8 => "__i8".apply(p),
            ValueType::u8 => "__u8".apply(p),
            ValueType::i16 => "__i16".apply(p),
            ValueType::u16 => "__u16".apply(p),
            ValueType::i32 => "__i32".apply(p),
            ValueType::u32 => "__u32".apply(p),
            ValueType::f32 => "__f32".apply(p),
            ValueType::f64 => "__f64".apply(p),
            ValueType::Str => "__str".apply(p),
            ValueType::String => "__string".apply(p),
            ValueType::char => "__char".apply(p),
            ValueType::Struct(s) => s.path(db).mangled(db).apply(p),
            ValueType::Enum(e) => e.path(db).mangled(db).apply(p),
        })
    }

    pub(super) fn uses_tethering(self) -> bool {
        match self {
            ValueType::bool
            | ValueType::char
            | ValueType::i8
            | ValueType::u8
            | ValueType::i16
            | ValueType::u16
            | ValueType::i32
            | ValueType::u32
            | ValueType::f32
            | ValueType::f64
            | ValueType::Str
            | ValueType::String => false,
            ValueType::Struct(_) | ValueType::Enum(_) => true,
        }
    }
}
