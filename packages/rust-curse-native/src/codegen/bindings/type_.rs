use crate::codegen::{bindings::tethering::TetheredType, db::Db, ir::type_::Type};

impl<'db> Type<'db> {
    /// Attempts to convert the type into a [tethered type][TetheredType].
    pub fn try_into_tethered_type(self, db: &'db dyn Db) -> Option<TetheredType<'db>> {
        match self {
            Type::Struct(id) => Some(id.into_tethered_type(db)),
            Type::Enum(id) => Some(id.into_tethered_type(db)),
            Type::Primitive(_) | Type::Str | Type::String => None,
        }
    }
}
