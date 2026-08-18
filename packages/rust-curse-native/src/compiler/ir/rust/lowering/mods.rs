use super::{ItemExt, RustdocId};
use crate::compiler::{
    db::Db,
    diag::DiagAccumulateExt,
    ir::rust::{Enum, Function, Member, Mod, ModParent, Struct},
};
use rustdoc_types as doc;

#[salsa::tracked]
impl<'db> Mod<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower(
        db: &'db dyn Db,
        id: RustdocId<'db>,
        parent: ModParent<'db>,
    ) -> Self {
        let item = id.item(db);
        Self::new(db, parent, item.name.as_ref().unwrap())
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_members(self, db: &'db dyn Db) -> Vec<Member<'db>> {
        let id = self.rustdoc_id(db);
        let item = id.item(db);

        let doc::ItemEnum::Module(mod_def) = &item.inner else {
            unreachable!("expected module, got {:?}", item.inner);
        };

        mod_def
            .items
            .iter()
            .filter_map(|item_id| {
                lower_mod_member(db, RustdocId::new(db, id.crate_name(db), *item_id), self)
            })
            .collect()
    }
}

fn lower_mod_member<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
    parent: Mod<'db>,
) -> Option<Member<'db>> {
    let summary = id.summary(db);

    match summary.kind {
        doc::ItemKind::Module => Some(Member::Mod(Mod::lower(db, id, ModParent::Mod(parent)))),
        doc::ItemKind::Struct => match Struct::lower(db, id, parent) {
            Ok(struct_) => Some(Member::Struct(struct_)),
            Err(err) => {
                err.accumulate(db);
                None
            }
        },
        doc::ItemKind::Enum => match Enum::lower(db, id, parent) {
            Ok(enum_) => Some(Member::Enum(enum_)),
            Err(err) => {
                err.accumulate(db);
                None
            }
        },
        doc::ItemKind::Function => match Function::lower(db, id, parent) {
            Ok(fn_) => Some(Member::Function(fn_)),
            Err(err) => {
                err.accumulate(db);
                None
            }
        },
        _ => None,
    }
}
