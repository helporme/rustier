use crate::compiler::{
    codegen::fragment::{FragmentOnce, Printer, frag, fragment},
    db::Db,
    ir::{
        rust::{FieldName, Type},
        shim::ShimType,
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

impl<'db> Type<'db> {
    pub(super) fn render_fqn(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        // FIXME: assumes that underlying type is not borrowed and it happens to be
        // true, but it may not always be in future
        assert!(!matches!(self, Type::Borrowed(_)));
        self.display(db)
    }

    pub(super) fn render_mangled_name(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            Type::bool => "__bool".apply(p),
            Type::i8 => "__i8".apply(p),
            Type::u8 => "__u8".apply(p),
            Type::i16 => "__i16".apply(p),
            Type::u16 => "__u16".apply(p),
            Type::i32 => "__i32".apply(p),
            Type::u32 => "__u32".apply(p),
            Type::f32 => "__f32".apply(p),
            Type::f64 => "__f64".apply(p),
            Type::Str => "__str".apply(p),
            Type::String => "__string".apply(p),
            Type::char => "__char".apply(p),
            Type::Struct(s) => s.path(db).mangled(db).apply(p),
            Type::Enum(e) => e.path(db).mangled(db).apply(p),
            Type::Borrowed(borrowed) => {
                if borrowed.is_mutable(db) {
                    "__mut".apply(p);
                } else {
                    "__ref".apply(p);
                }
                borrowed.inner(db).render_mangled_name(db).apply(p);
            }
        })
    }

    pub(super) fn uses_tethering(self, db: &'db dyn Db) -> bool {
        match self {
            Type::bool
            | Type::char
            | Type::i8
            | Type::u8
            | Type::i16
            | Type::u16
            | Type::i32
            | Type::u32
            | Type::f32
            | Type::f64
            | Type::Str
            | Type::String => false,
            Type::Struct(_) | Type::Enum(_) => true,
            Type::Borrowed(borrowed) => borrowed.inner(db).uses_tethering(db),
        }
    }
}
