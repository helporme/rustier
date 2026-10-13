use crate::compiler::{
    codegen::fragment::{FragmentOnce, doubles, frag, fragment},
    db::Db,
    ir::facade::{
        FacadeGeneric,
        type_ts::{FacadeTypeNameTs, FacadeTypeTs},
    },
};

impl<'db> FacadeTypeTs<'db> {
    pub(super) fn render_ts(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            FacadeTypeTs::boolean => frag!(boolean).apply(p),
            FacadeTypeTs::string => frag!(string).apply(p),
            FacadeTypeTs::number => frag!(number).apply(p),
            FacadeTypeTs::Class { inner, coerced } => {
                inner
                    .type_name(db)
                    .render_ts(db, coerced.then_some("Coerce"))
                    .apply(p);
            }
        })
    }
}

impl<'db> FacadeTypeNameTs<'db> {
    pub(super) fn render_ts(
        self,
        db: &'db dyn Db,
        projection: Option<&'static str>,
    ) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            self.name(db).apply(p);

            if let Some(projection) = projection {
                frag!(.{projection}).apply(p);
            }

            if let Some(args) = self.args(db) {
                frag!(<{render_inline_list(args.iter().map(|arg| arg.render_ts(db)))}>).apply(p);
            }
        })
    }
}

impl<'db> FacadeGeneric<'db> {
    pub(super) fn render_ts(&'db self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            Self::Symbol(symbol) => doubles(symbol).apply(p),
            Self::Class(class) => {
                if class.is_template_instance(db) {
                    class.type_name(db).render_ts(db, Some("Class")).apply(p);
                } else {
                    frag!(typeof ~{class.type_name(db).render_ts(db, None)}).apply(p);
                }
            }
        })
    }
}

pub(super) fn render_inline_list<'a, F: FragmentOnce + 'a>(
    items: impl IntoIterator<Item = F, IntoIter: 'a>,
) -> impl FragmentOnce + 'a {
    let items = items.into_iter();

    fragment(move |p| {
        for (index, item) in items.enumerate() {
            if index > 0 {
                frag!(,~).apply(p);
            }

            item.apply(p);
        }
    })
}
