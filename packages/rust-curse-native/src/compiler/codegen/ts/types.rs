use crate::compiler::{
    codegen::fragment::{FragmentOnce, frag, fragment},
    db::Db,
    ir::decals::DecalType,
};

impl<'db> DecalType<'db> {
    pub(super) fn render_ts(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            DecalType::boolean => frag!(boolean).apply(p),
            DecalType::string => frag!(string).apply(p),
            DecalType::number => frag!(number).apply(p),
            DecalType::Class(class) => frag!({ class.name(db) }).apply(p),
        })
    }
}
