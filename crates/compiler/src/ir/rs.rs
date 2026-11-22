//! Rust modules in the active analysis workspace.

use ra_ap_base_db::Crate;
use ra_ap_hir::{InFile, ModuleDef, ScopeDef};
use ra_ap_hir_def::DefWithBodyId;
use ra_ap_syntax::{AstPtr, ast};
use std::collections::HashSet;

use crate::db::{Db, VirtFile};

mod js_calls;

/// The Rust side of the program.
///
/// Its root identifies the dependency graph and settings used to analyze Rust bodies.
#[salsa::interned(no_lifetime)]
pub struct RsProgram {
    root: Crate,
}

#[salsa::tracked]
impl RsProgram {
    /// Modules in the Rust dependency graph, excluding the generated analysis root.
    #[salsa::tracked(returns(ref))]
    pub fn modules(self, db: &dyn Db) -> Vec<RsModule<'_>> {
        self.dependencies(db)
            .iter()
            .flat_map(|krate| krate.modules(db))
            .map(|module| RsModule::new(db, self, module))
            .collect()
    }

    /// Dependency modules that can reach `js!`, including via a re-export.
    #[salsa::tracked(returns(ref))]
    pub fn modules_using_js(self, db: &dyn Db) -> Vec<RsModule<'_>> {
        let Some(mac) = self.js_macro_definition(db) else {
            return Vec::new();
        };
        let consumers: HashSet<_> = mac
            .module(db)
            .krate(db)
            .transitive_reverse_dependencies(db)
            .collect();
        self.dependencies(db)
            .iter()
            .filter(|&krate| consumers.contains(krate))
            .flat_map(|krate| krate.modules(db))
            .map(|module| RsModule::new(db, self, module))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    fn dependencies(self, db: &dyn Db) -> Vec<ra_ap_hir::Crate> {
        let root = self.root(db);
        root.transitive_deps(db)
            .into_iter()
            .filter(|&krate| krate != root)
            .map(ra_ap_hir::Crate::from)
            .collect()
    }

    #[salsa::tracked]
    fn facade(self, db: &dyn Db) -> Option<ra_ap_hir::Module> {
        ra_ap_hir::Crate::from(self.root(db))
            .dependencies(db)
            .into_iter()
            .find(|dep| dep.name.as_str() == "rustier")
            .map(|dep| dep.krate.root_module(db))
    }

    #[salsa::tracked]
    fn js_macro_definition(self, db: &dyn Db) -> Option<ra_ap_hir::Macro> {
        self.facade(db)?
            .scope(db, None)
            .into_iter()
            .find_map(|(name, def)| match def {
                ScopeDef::ModuleDef(ModuleDef::Macro(mac)) if name.as_str() == "js" => Some(mac),
                _ => None,
            })
    }

    #[salsa::tracked]
    fn js_value_struct(self, db: &dyn Db) -> Option<ra_ap_hir_def::StructId> {
        self.facade(db)?
            .scope(db, None)
            .into_iter()
            .find_map(|(name, def)| match def {
                ScopeDef::ModuleDef(ModuleDef::Adt(ra_ap_hir::Adt::Struct(s)))
                    if name.as_str() == "JsValue" =>
                {
                    Some(s.into())
                }
                _ => None,
            })
    }

    pub(crate) fn is_js_value(self, db: &dyn Db, ty: ra_ap_hir_ty::next_solver::Ty<'_>) -> bool {
        matches!(ty.as_adt(), Some((ra_ap_hir_def::AdtId::StructId(id), _))
            if Some(id) == self.js_value_struct(db))
    }

    /// A virtual source file analyzed with this program's dependencies, target and cfg.
    #[salsa::tracked]
    pub(crate) fn probe_crate(self, db: &dyn Db, file: VirtFile) -> Crate {
        let root = self.root(db);
        let mut data = root.data(db).clone();
        data.root_file_id = file.file_id();
        Crate::builder(
            data,
            root.extra_data(db).clone(),
            root.workspace_data(db).clone(),
            root.cfg_options(db).clone(),
            root.env(db).clone(),
        )
        .new(db)
    }
}

/// A Rust module, including inline modules which share a file with their parent.
#[salsa::interned]
pub struct RsModule<'db> {
    program: RsProgram,
    src: ra_ap_hir::Module,
}

/// One `js!` occurrence in dependency source or a generated `rs` analysis file.
#[salsa::interned]
pub struct JsMacroSrc<'db> {
    /// The complete invocation in its originating Rust file.
    call: InFile<AstPtr<ast::MacroCall>>,
    /// The body in which rust-analyzer infers the interpolation expressions.
    body: DefWithBodyId,
}
