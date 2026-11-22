use ra_ap_hir::HasSource;
use ra_ap_hir_def::{
    DefWithBodyId, ModuleDefId, expr_store::ExpressionStore, nameres::crate_def_map,
};
use ra_ap_hir_ty::{
    InferenceResult,
    next_solver::{DbInterner, FnSig, Ty, TyKind},
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};

use super::{
    classify_type,
    compile_errors::{self, CompileError},
};
use crate::{
    db::{Db, VirtFile},
    ir::rs::JsMacroSrc,
    ir::runtime::{HandleBorrow, RsMacro, RsPart, RsType},
};

#[salsa::tracked]
impl<'db> RsMacro<'db> {
    /// Checks the macro's rust code for compile errors using rust-analyzer.
    ///
    /// It's not necessary capable of detecting all compile errors, but it can
    /// catch most common ones.
    #[salsa::tracked(returns(ref))]
    pub fn check_compile_errors(self, db: &'db dyn Db) -> Option<Vec<CompileError>> {
        let source = self.render_analysis_rs(db, Some(self.inputs(db)), Some(self.ret_type(db)));
        let file = VirtFile::new(db, source.text.as_str(), source.source_map);
        let crate_ = self.program(db).probe_crate(db, file);

        compile_errors::check(db, crate_)
    }

    /// Whether rust-analyzer found no compilation errors in this macro.
    pub fn is_compilable(self, db: &'db dyn Db) -> bool {
        self.check_compile_errors(db).is_none()
    }

    #[salsa::tracked(returns(ref))]
    pub fn inputs(self, db: &'db dyn Db) -> Vec<RsType> {
        match infer_closure(db, self.input_probe(db)) {
            Some((callable, _)) => callable
                .inputs()
                .iter()
                .copied()
                .map(|ty| classify_type(db, self.program(db), ty))
                .collect(),
            None => self
                .body(db)
                .iter()
                .filter_map(|part| match part {
                    RsPart::Input => Some(RsType::JsValue),
                    RsPart::Text(_) => None,
                })
                .collect(),
        }
    }

    #[salsa::tracked]
    pub fn ret_type(self, db: &'db dyn Db) -> RsType {
        infer_closure(db, self.output_probe(db)).map_or(RsType::JsValue, |(callable, _)| {
            let output = callable.output();

            let type_ = match result_types(db, output) {
                Some((value, _)) if matches!(value.inner().internee, TyKind::Error(_)) => {
                    RsType::Void
                }
                Some((value, _)) => classify_type(db, self.program(db), value),
                None => classify_type(db, self.program(db), output),
            };

            match type_ {
                RsType::RustHandle {
                    borrow: HandleBorrow::Shared | HandleBorrow::Mutable,
                } => RsType::JsValue,
                _ => type_,
            }
        })
    }

    /// Whether the macro can return an error.
    #[salsa::tracked]
    pub fn can_return_error(self, db: &'db dyn Db) -> bool {
        self.returns_result(db)
            || self
                .inputs(db)
                .iter()
                .any(|ty| matches!(ty, RsType::RustHandle { .. }))
    }

    #[salsa::tracked]
    pub(crate) fn returns_result(self, db: &'db dyn Db) -> bool {
        infer_closure(db, self.output_probe(db)).is_some_and(|(callable, has_try)| {
            has_try || result_types(db, callable.output()).is_some()
        })
    }

    /// Whether rust-analyzer inferred the error type of the macro's `Result`.
    ///
    /// Unlike `can_return_error`, which determines whether the macro can return an error,
    /// this functions returns whether the rust-analyzer knows concrete error type for the
    /// macro's `Result`. Usually if this function returns `false`, codegen needs to use Box<dyn Error + ...>
    /// or similar to handle the error.
    #[salsa::tracked]
    pub(crate) fn has_inferred_error_type(self, db: &'db dyn Db) -> bool {
        infer_closure(db, self.output_probe(db)).is_some_and(|(callable, _)| {
            result_types(db, callable.output())
                .is_some_and(|(_, error)| !error.references_non_lt_error())
        })
    }

    #[salsa::tracked]
    fn input_probe(self, db: &'db dyn Db) -> ra_ap_base_db::Crate {
        let source = self.render_analysis_rs(db, None, None);

        let file = VirtFile::new(db, source.text.as_str(), source.source_map);
        self.program(db).probe_crate(db, file)
    }

    /// Resolved `js!` calls in the analyzed body. Invalid `rs` bodies generate no imports.
    #[salsa::tracked(returns(ref))]
    pub fn js_macros(self, db: &'db dyn Db) -> Vec<JsMacroSrc<'db>> {
        if !self
            .body(db)
            .iter()
            .any(|part| matches!(part, RsPart::Text(s) if s.contains('!')))
            || !self.is_compilable(db)
        {
            return Vec::new();
        }
        self.program(db).probe_js_calls(db, self.output_probe(db))
    }

    #[salsa::tracked]
    fn output_probe(self, db: &'db dyn Db) -> ra_ap_base_db::Crate {
        let types = self.inputs(db);
        let source = self.render_analysis_rs(db, Some(types), None);

        let file = VirtFile::new(db, source.text.as_str(), source.source_map);
        self.program(db).probe_crate(db, file)
    }
}

fn infer_closure<'db>(db: &'db dyn Db, crate_: ra_ap_base_db::Crate) -> Option<(FnSig<'db>, bool)> {
    let def_map = crate_def_map(db, crate_);

    let (function_id, source) = def_map.modules[def_map.root]
        .scope
        .declarations()
        .find_map(|item| {
            let ModuleDefId::FunctionId(id) = item else {
                return None;
            };

            let function: ra_ap_hir::Function = id.into();
            let source = function.source(db)?;

            source
                .value
                .name()
                .is_some_and(|name| name.text() == "__rustier_probe")
                .then_some((id, source))
        })?;

    let closure = source
        .value
        .syntax()
        .descendants()
        .find_map(ast::ClosureExpr::cast)?;

    let has_try = closure
        .syntax()
        .descendants()
        .filter_map(ast::TryExpr::cast)
        .any(|expr| {
            expr.syntax()
                .ancestors()
                .skip(1)
                .take_while(|node| node != closure.syntax())
                .all(|node| {
                    !ast::Fn::can_cast(node.kind())
                        && !ast::ClosureExpr::can_cast(node.kind())
                        && !ast::BlockExpr::cast(node).is_some_and(|block| {
                            block.async_token().is_some()
                                || block.const_token().is_some()
                                || block.try_block_modifier().is_some()
                        })
                })
        });
    let expr = ast::Expr::ClosureExpr(closure);
    let owner = DefWithBodyId::FunctionId(function_id);
    let (_, source_map) = ExpressionStore::with_source_map(db, owner.into());
    let expr_id = source_map.node_expr(ra_ap_hir::InFile::new(source.file_id, &expr))?;

    let inference = InferenceResult::of(db, owner);
    let ty = inference.expr_or_pat_ty(expr_id);
    let callable = ty
        .callable_sig(DbInterner::new_with(db, crate_))?
        .skip_binder();

    Some((callable, has_try))
}

fn result_types<'db>(db: &'db dyn Db, ty: Ty<'db>) -> Option<(Ty<'db>, Ty<'db>)> {
    let (adt, args) = ty.as_adt()?;
    let ra_ap_hir_def::AdtId::EnumId(id) = adt else {
        return None;
    };
    let enum_: ra_ap_hir::Enum = id.into();
    let module = enum_.module(db);
    let crate_ = module.krate(db);

    (crate_.is_builtin(db)
        && enum_.name(db).display(db, crate_.edition(db)).to_string() == "Result"
        && module
            .path_segments(db)
            .map(|name| name.display(db, crate_.edition(db)).to_string())
            .eq(["result"]))
    .then(|| {
        let mut types = args.types();

        (types.next().unwrap(), types.next().unwrap())
    })
}
