use ra_ap_hir::{Adt, HasAttrs as _, HasVisibility as _, ModuleDef, Visibility};

use super::{file_path, lower_enum, lower_function, lower_span, lower_struct};
use crate::compiler::{
    FilePath,
    db::Db,
    diag::DiagAccumulateExt as _,
    ir::rust::{Crate, Member, Mod, ModParent, Span},
};

#[salsa::tracked]
impl<'db> Mod<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_parent(self, db: &'db dyn Db) -> ModParent<'db> {
        let hir = self.hir(db);
        match hir.parent(db) {
            Some(parent) => ModParent::Mod(Self::new(db, parent)),
            None => ModParent::Crate(Crate::new(db, hir.krate(db))),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_name(self, db: &'db dyn Db) -> String {
        let hir = self.hir(db);
        hir.name(db)
            .map(|name| name.display(db, hir.krate(db).edition(db)).to_string())
            .or_else(|| hir.krate(db).display_name(db).map(|name| name.to_string()))
            .unwrap_or_default()
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_members(self, db: &'db dyn Db) -> Vec<Member<'db>> {
        self.hir(db)
            .declarations(db)
            .into_iter()
            .filter(|item| item.visibility(db) == Visibility::Public)
            .filter_map(|item| match item {
                ModuleDef::Module(module) => Some(Member::Mod(Mod::new(db, module))),
                ModuleDef::Function(function) => lower_function(db, function)
                    .map(Member::Function)
                    .map_err(|error| (*error).accumulate(db))
                    .ok(),
                ModuleDef::Adt(Adt::Struct(struct_)) => lower_struct(db, struct_)
                    .map(Member::Struct)
                    .map_err(|error| (*error).accumulate(db))
                    .ok(),
                ModuleDef::Adt(Adt::Enum(enum_)) => lower_enum(db, enum_)
                    .map(Member::Enum)
                    .map_err(|error| (*error).accumulate(db))
                    .ok(),
                _ => None,
            })
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_docs(self, db: &'db dyn Db) -> Option<String> {
        self.hir(db).hir_docs(db).map(|docs| docs.docs().to_owned())
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_span(self, db: &'db dyn Db) -> Option<Span> {
        let source = self.hir(db).definition_source_range(db);
        lower_span(
            db,
            source.file_id,
            source.value.start().into(),
            source.value.end().into(),
        )
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_file_path(self, db: &'db dyn Db) -> FilePath {
        let source_file = self
            .hir(db)
            .definition_source_file_id(db)
            .original_file(db)
            .file_id(db);
        file_path(db, source_file).unwrap_or_else(|| self.path(db).crate_(db).dir(db).to_owned())
    }
}
