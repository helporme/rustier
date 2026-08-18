use std::collections::HashMap;

use crate::compiler::{
    db::Db,
    ir::doc::RustdocCrates,
    ir::rust::{AsItem, Enum, Field, Function, ItemPath, Mod, Span, Struct, Variant},
};
use rustdoc_types as doc;

pub mod crates;
pub mod enums;
pub mod fields;
pub mod functions;
pub mod mods;
pub mod shapes;
pub mod structs;
pub mod types;

pub(in crate::compiler::ir::rust) trait ItemExt<'db>:
    AsItem<'db>
{
    fn kind() -> doc::ItemKind;

    fn rustdoc_id(self, db: &'db dyn Db) -> RustdocId<'db> {
        lookup_rustdoc_id(db, self.path(db), Self::kind())
    }

    fn lower_span(self, db: &'db dyn Db) -> Option<&'db Span> {
        let id = self.rustdoc_id(db);
        let item = id.item(db);

        item.span.as_ref().map(|span| {
            let crate_name = id.crate_name(db);
            RustdocSpan::new(db, span, crate_name).lower(db)
        })
    }

    fn lower_docs(self, db: &'db dyn Db) -> Option<&'db String> {
        let id = self.rustdoc_id(db);
        let item = id.item(db);
        item.docs.as_ref()
    }
}

macro_rules! impl_item_ext {
    ($type:ident, $kind:expr) => {
        impl<'db> ItemExt<'db> for $type<'db> {
            fn kind() -> doc::ItemKind {
                $kind
            }
        }
    };
}
impl_item_ext!(Mod, doc::ItemKind::Module);
impl_item_ext!(Struct, doc::ItemKind::Struct);
impl_item_ext!(Enum, doc::ItemKind::Enum);
impl_item_ext!(Variant, doc::ItemKind::Variant);
impl_item_ext!(Field, doc::ItemKind::StructField);
impl_item_ext!(Function, doc::ItemKind::Function);

#[salsa::interned]
struct RustdocSpan<'db> {
    #[returns(ref)]
    pub inner: doc::Span,
    #[returns(ref)]
    pub crate_name: String,
}

#[salsa::tracked]
impl<'db> RustdocSpan<'db> {
    #[salsa::tracked(returns(ref))]
    fn lower(self, db: &'db dyn Db) -> Span {
        let span = self.inner(db);
        let crate_dir = &RustdocCrates::get(db)
            .crate_data(db, self.crate_name(db))
            .unwrap()
            .dir;

        Span {
            abs_path: if span.filename.is_absolute() {
                span.filename.clone()
            } else {
                crate_dir.join(&span.filename)
            },
            src_path: span.filename.clone(),
            begin: span.begin,
            end: span.end,
        }
    }
}

fn lookup_rustdoc_id<'db>(
    db: &'db dyn Db,
    path: ItemPath<'db>,
    kind: doc::ItemKind,
) -> RustdocId<'db> {
    let crate_name = path.crate_(db).name(db);
    let segments = path.segments(db);
    let lookup = RustdocIdKey::new(db, crate_name, segments, kind);

    rustdoc_backmap(db).get(&lookup).copied().unwrap()
}

#[salsa::interned]
struct RustdocIdKey {
    crate_name: String,
    path: Vec<String>,
    kind: doc::ItemKind,
}

#[salsa::interned]
pub(in crate::compiler::ir::rust) struct RustdocId {
    #[returns(ref)]
    pub crate_name: String,
    pub inner: doc::Id,
}

impl<'db> RustdocId<'db> {
    pub fn item(self, db: &'db dyn Db) -> &'db doc::Item {
        let crate_ = self.crate_(db);
        let id = self.inner(db);

        crate_.index.get(&id).unwrap()
    }

    pub fn summary(self, db: &'db dyn Db) -> &'db doc::ItemSummary {
        let crate_ = self.crate_(db);
        let id = self.inner(db);
        crate_.paths.get(&id).unwrap()
    }

    pub fn crate_(self, db: &'db dyn Db) -> &'db doc::Crate {
        let rustdoc = RustdocCrates::get(db);

        // Crate must exist in input rustdoc, otherwise it won't be requested;
        // And if it doesn't exist it's unrecoverable
        rustdoc.crate_(db, self.crate_name(db)).unwrap()
    }
}

#[salsa::tracked(returns(ref))]
fn rustdoc_backmap<'db>(db: &'db dyn Db) -> HashMap<RustdocIdKey<'db>, RustdocId<'db>> {
    let rustdoc = RustdocCrates::get(db);
    let mut backmap = HashMap::new();

    for (crate_name, crate_) in rustdoc.crates(db) {
        for (idx, item_summary) in &crate_.body.paths {
            if !crate_.body.index.contains_key(idx) {
                continue;
            }

            let lookup = RustdocIdKey::new(db, crate_name, &item_summary.path, item_summary.kind);
            let rustdoc_id = RustdocId::new(db, crate_name, idx);

            backmap.insert(lookup, rustdoc_id);
        }
    }

    backmap
}

/// An interned type to a specific rustdoc item in a specific crate.
#[salsa::interned]
pub struct RustdocType<'db> {
    #[returns(ref)]
    pub crate_name: String,
    #[returns(ref)]
    pub inner: doc::Type,
}

impl<'db> RustdocType<'db> {
    pub fn crate_(&self, db: &'db dyn Db) -> &'db doc::Crate {
        let rustdoc = RustdocCrates::get(db);
        &rustdoc.crate_data(db, self.crate_name(db)).unwrap().body
    }
}
