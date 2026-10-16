use crate::compiler::{
    codegen::fragment::{FragmentOnce, Printer, fragment},
    db::Db,
    ir::rust::{FieldName, Type, TypeKind},
};

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
        self.display(db)
    }

    pub(super) fn render_mangled_name(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self.kind(db) {
            TypeKind::Unit => "__unit".apply(p),
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
            TypeKind::Generic(_) => unreachable!("unsupported type cannot reach runtime codegen"),
        })
    }
}
