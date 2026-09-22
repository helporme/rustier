use crate::compiler::{
    codegen::fragment::{FragmentOnce, Printer, frag, fragment},
    db::Db,
    ir::{
        rust::{FieldName, Type, TypeKind},
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
        assert!(!matches!(self.kind(db), TypeKind::Borrowed(_)));
        self.assert_shim_supported(db);
        self.display(db)
    }

    pub(super) fn render_mangled_name(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        self.assert_shim_supported(db);
        fragment(move |p| match self.kind(db) {
            TypeKind::bool => "__bool".apply(p),
            TypeKind::i8 => "__i8".apply(p),
            TypeKind::u8 => "__u8".apply(p),
            TypeKind::i16 => "__i16".apply(p),
            TypeKind::u16 => "__u16".apply(p),
            TypeKind::i32 => "__i32".apply(p),
            TypeKind::u32 => "__u32".apply(p),
            TypeKind::f32 => "__f32".apply(p),
            TypeKind::f64 => "__f64".apply(p),
            TypeKind::Str => "__str".apply(p),
            TypeKind::String => "__string".apply(p),
            TypeKind::char => "__char".apply(p),
            TypeKind::Struct(s) => s.path(db).mangled(db).apply(p),
            TypeKind::Enum(e) => e.path(db).mangled(db).apply(p),
            TypeKind::Borrowed(borrowed) => {
                if borrowed.is_mutable(db) {
                    "__mut".apply(p);
                } else {
                    "__ref".apply(p);
                }
                borrowed.inner(db).render_mangled_name(db).apply(p);
            }
            TypeKind::Generic(_) => unreachable!("unsupported type cannot reach shim codegen"),
        })
    }

    pub(super) fn uses_tethering(self, db: &'db dyn Db) -> bool {
        self.assert_shim_supported(db);
        match self.kind(db) {
            TypeKind::bool
            | TypeKind::char
            | TypeKind::i8
            | TypeKind::u8
            | TypeKind::i16
            | TypeKind::u16
            | TypeKind::i32
            | TypeKind::u32
            | TypeKind::f32
            | TypeKind::f64
            | TypeKind::Str
            | TypeKind::String => false,
            TypeKind::Struct(_) | TypeKind::Enum(_) => true,
            TypeKind::Borrowed(borrowed) => borrowed.inner(db).uses_tethering(db),
            TypeKind::Generic(_) => unreachable!("unsupported type cannot reach shim codegen"),
        }
    }
}
