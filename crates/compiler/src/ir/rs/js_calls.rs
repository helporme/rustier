use ra_ap_base_db::Crate;
use ra_ap_hir::{InFile, ModuleDef, ScopeDef, Semantics};
use ra_ap_hir_def::{
    DefWithBodyId, MacroId, ModuleDefId, expr_store::ExpressionStore, nameres::crate_def_map,
};
use ra_ap_hir_ty::{InferenceResult, next_solver::Ty};
use ra_ap_syntax::{
    AstNode, AstPtr, WalkEvent,
    ast::{self, HasArgList},
};

use super::{JsMacroSrc, RsModule, RsProgram};
use crate::db::Db;

impl RsProgram {
    /// Finds occurrences in the virtual crate used to type-check an inline Rust body.
    pub(crate) fn probe_js_calls(self, db: &dyn Db, probe: Crate) -> Vec<JsMacroSrc<'_>> {
        let inline = ra_ap_hir::Crate::from(probe)
            .dependencies(db)
            .into_iter()
            .find(|dep| dep.name.as_str() == "rustier_js_proc")
            .and_then(|dep| {
                dep.krate
                    .root_module(db)
                    .scope(db, None)
                    .into_iter()
                    .find_map(|(name, def)| match def {
                        ScopeDef::ModuleDef(ModuleDef::Macro(mac))
                            if name.as_str() == "__js_in_rs" =>
                        {
                            Some(mac)
                        }
                        _ => None,
                    })
            });
        let definitions = [inline, self.js_macro_definition(db)]
            .map(|mac| mac.map(|mac| MacroId::from(mac).definition(db)));
        let sema = Semantics::new_dyn(db);
        let modules = crate_def_map(db, probe);
        let mut sites = Vec::new();
        for item in modules.modules[modules.root].scope.declarations() {
            let ModuleDefId::FunctionId(id) = item else {
                continue;
            };
            let function: ra_ap_hir::Function = id.into();
            let source = sema.source(function).unwrap();
            let owner = DefWithBodyId::FunctionId(id);
            let (_, map) = ExpressionStore::with_source_map(db, owner.into());
            for call in source
                .value
                .syntax()
                .descendants()
                .filter_map(ast::MacroCall::cast)
            {
                let ptr = InFile::new(source.file_id, AstPtr::new(&call));
                if let Some(expansion) = map.expansion(InFile::new(source.file_id, &call))
                    && definitions.contains(&Some(expansion.loc(db).def))
                {
                    sites.push(JsMacroSrc::new(db, ptr, owner));
                }
            }
        }
        sites
    }
}

#[salsa::tracked]
impl<'db> RsModule<'db> {
    /// Resolved `js!` occurrences in this module, excluding its child modules.
    #[salsa::tracked(returns(ref))]
    pub fn js_macros(self, db: &'db dyn Db) -> Vec<JsMacroSrc<'db>> {
        let Some(js) = self.program(db).js_macro_definition(db) else {
            return Vec::new();
        };
        let sema = Semantics::new_dyn(db);
        let source = sema.module_definition_node(self.src(db));
        let mut walk = source.value.preorder();
        let mut sites = Vec::new();

        while let Some(event) = walk.next() {
            let WalkEvent::Enter(node) = event else {
                continue;
            };
            if node != source.value && ast::Module::can_cast(node.kind()) {
                walk.skip_subtree();
                continue;
            }
            let Some(call) = ast::MacroCall::cast(node) else {
                continue;
            };
            if sema.resolve_macro_call(&call) != Some(js) {
                continue;
            }

            let owner = sema.store_owner_for(InFile::new(source.file_id, call.syntax()));
            if let Some(ra_ap_hir::ExpressionStoreOwner::Body(owner)) = owner
                && let Ok(owner) = DefWithBodyId::try_from(owner)
            {
                sites.push(JsMacroSrc::new(
                    db,
                    InFile::new(source.file_id, AstPtr::new(&call)),
                    owner,
                ));
            }
        }
        sites
    }
}

impl<'db> JsMacroSrc<'db> {
    /// The positional JavaScript body and its Rust interpolation expressions.
    pub(crate) fn parse(self, db: &'db dyn Db) -> Option<rustier_js_syntax::ParsedJsMacro> {
        let tree = Semantics::new_dyn(db).to_node(self.call(db)).token_tree()?;
        let source = tree.syntax().text().to_string();
        rustier_js_syntax::ParsedJsMacro::parse(
            source.get(1..source.len().checked_sub(1)?)?.parse().ok()?,
        )
        .ok()
    }

    /// Rust types at this call site, before applying any runtime conversion rules.
    pub(crate) fn inferred_inputs(self, db: &'db dyn Db) -> Option<Vec<Ty<'db>>> {
        let sema = Semantics::new_dyn(db);
        let ptr = self.call(db);
        let call = sema.to_node(ptr);
        let owner = self.body(db);
        let (_, map) = ExpressionStore::with_source_map(db, owner.into());
        let file = map.expansion(InFile::new(ptr.file_id, &call))?.into();
        let expansion = sema.parse_or_expand(file);
        let root = expansion.descendants().find_map(ast::Expr::cast)?;
        let (file, root) = if let ast::Expr::MacroExpr(forward) = root {
            let call = forward.macro_call()?;
            let inner = map.expansion(InFile::new(file, &call))?.into();
            let expr = sema.parse_or_expand(inner).descendants().find_map(ast::Expr::cast)?;
            (inner, expr)
        } else {
            (file, root)
        };
        let constructor = match root {
            ast::Expr::CallExpr(call) => call,
            ast::Expr::MethodCallExpr(at) => ast::CallExpr::cast(at.receiver()?.syntax().clone())?,
            _ => return None,
        };
        let inference = InferenceResult::of(db, owner);
        constructor
            .arg_list()?
            .args()
            .map(|arg| {
                let id = map.node_expr(InFile::new(file, &arg))?;
                Some(inference.expr_or_pat_ty(id))
            })
            .collect()
    }
}
