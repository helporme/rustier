use crate::{
    compiler::{
        codegen::{
            api::Api,
            fragment::{FragmentOnce, Printer, fragment},
        },
        db::Db,
        ir::{
            rust::{AsItem, FieldName, Type, ValueType},
            shim::{ShimType, ShimValueType},
        },
    },
    frag,
};

impl<'db> ShimValueType<'db> {
    pub(super) fn render_type_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            ShimValueType::bool => "bool".apply(p),
            ShimValueType::i8 => "i8".apply(p),
            ShimValueType::u8 => "u8".apply(p),
            ShimValueType::i16 => "i16".apply(p),
            ShimValueType::u16 => "u16".apply(p),
            ShimValueType::i32 => "i32".apply(p),
            ShimValueType::u32 => "u32".apply(p),
            ShimValueType::f64 => "f64".apply(p),
            ShimValueType::String => "String".apply(p),
            ShimValueType::JsAny => Api::get(db).js_type(db).apply(p),
            ShimValueType::Class(c) => c.symbol(db).apply(p),
        })
    }

    pub(super) fn uses_tethering(&self) -> bool {
        match self {
            ShimValueType::Class(_) => true,
            ShimValueType::bool
            | ShimValueType::i8
            | ShimValueType::u8
            | ShimValueType::i16
            | ShimValueType::u16
            | ShimValueType::i32
            | ShimValueType::u32
            | ShimValueType::f64
            | ShimValueType::String
            | ShimValueType::JsAny => false,
        }
    }
}

impl<'db> ShimType<'db> {
    pub(super) fn render_type_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            ShimType::Value(ty) => ty.render_type_decl(db).apply(p),
            ShimType::Borrowed(ty) => frag!(&{ ty.render_type_decl(db) }).apply(p),
        })
    }
}

impl<'a> FragmentOnce for &'a FieldName {
    fn apply(self, p: &mut Printer) {
        match self {
            FieldName::Idx(idx) => idx.apply(p),
            FieldName::Name(name) => name.apply(p),
        }
    }
}

impl<'db> ValueType<'db> {
    pub(super) fn render_type_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
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

impl<'db> Type<'db> {
    pub(super) fn render_type_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            Type::Value(type_) => type_.render_type_decl(db).apply(p),
            Type::Borrowed { is_mutable, inner } => {
                if is_mutable {
                    "&mut ".apply(p)
                } else {
                    "&".apply(p)
                }
                inner.render_type_decl(db).apply(p)
            }
        })
    }
}
