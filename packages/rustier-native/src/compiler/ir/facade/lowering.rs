use crate::compiler::{
    db::Db,
    ir::{
        facade::FacadeType,
        rust::{Type, TypeKind},
    },
};

impl<'db> Type<'db> {
    pub(super) fn lower_facade(self, db: &'db dyn Db) -> FacadeType<'db> {
        if let Some(class) = self.interface_class(db) {
            return FacadeType::Class(class.facade(db));
        }

        match self.kind(db) {
            TypeKind::bool => FacadeType::boolean,
            TypeKind::char
            | TypeKind::i8
            | TypeKind::u8
            | TypeKind::i16
            | TypeKind::u16
            | TypeKind::i32
            | TypeKind::u32
            | TypeKind::f32
            | TypeKind::f64 => FacadeType::number,
            TypeKind::Str | TypeKind::String => FacadeType::string,
            TypeKind::Borrowed(borrowed) => borrowed.inner(db).lower_facade(db),
            TypeKind::Struct(_) | TypeKind::Enum(_) | TypeKind::Generic(_) => {
                unreachable!("unsupported Rust types do not enter the facade")
            }
        }
    }
}
