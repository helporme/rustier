//! Analysis running on macros to derive it's input/output types.

use ra_ap_base_db::Crate;
use ra_ap_hir::HasSource;
use ra_ap_hir_def::{
    DefWithBodyId, ModuleDefId,
    builtin_type::{BuiltinFloat, BuiltinInt, BuiltinType, BuiltinUint},
    expr_store::ExpressionStore,
    nameres::crate_def_map,
};
use ra_ap_hir_ty::{
    InferenceResult,
    next_solver::{DbInterner, FnSig, Ty},
};
use ra_ap_syntax::{AstNode, ast, ast::HasName};
use salsa::Setter;

use super::{RsMacro, RsPart, RsType};
use crate::db::{Db, VirtFile};

/// The dependency and source configuration used by all runtime type queries.
#[salsa::input(singleton)]
pub struct RuntimeAnalysis {
    template: Crate,
}

impl RuntimeAnalysis {
    /// Sets the macro analysis in a context of a given template crate.
    pub fn set(db: &mut dyn Db, template: Crate) -> Self {
        match Self::try_get(db) {
            Some(input) => {
                if input.template(db) != template {
                    input.set_template(db).to(template);
                }

                input
            }
            None => Self::new(db, template),
        }
    }
}

#[salsa::tracked]
impl<'db> RsMacro<'db> {
    #[salsa::tracked]
    fn analysis_file_for_inputs(self, db: &'db dyn Db) -> VirtFile {
        let source = self.render_analysis_rs(db, None);

        VirtFile::new(db, source.as_str())
    }

    #[salsa::tracked]
    fn analysis_file_for_output(self, db: &'db dyn Db) -> VirtFile {
        let types = self.infer_input_types(db);
        let source = self.render_analysis_rs(db, Some(types));

        VirtFile::new(db, source.as_str())
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn infer_input_types(self, db: &'db dyn Db) -> Vec<RsType> {
        let file = self.analysis_file_for_inputs(db);

        match infer_closure(db, file) {
            Some(callable) => callable
                .inputs()
                .iter()
                .copied()
                .map(classify_type)
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
    pub(super) fn infer_ret_type(self, db: &'db dyn Db) -> RsType {
        let file = self.analysis_file_for_output(db);

        infer_closure(db, file).map_or(RsType::JsValue, |callable| classify_type(callable.output()))
    }
}

fn infer_closure<'db>(db: &'db dyn Db, file: VirtFile) -> Option<FnSig<'db>> {
    let crate_ = single_file_crate(db, file);
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

    let expr = ast::Expr::ClosureExpr(closure);
    let owner = DefWithBodyId::FunctionId(function_id);
    let (_, source_map) = ExpressionStore::with_source_map(db, owner.into());
    let expr_id = source_map.node_expr(ra_ap_hir::InFile::new(source.file_id, &expr))?;

    let inference = InferenceResult::of(db, owner);
    let ty = inference.expr_or_pat_ty(expr_id);
    let callable = ty
        .callable_sig(DbInterner::new_with(db, crate_))?
        .skip_binder();

    Some(callable)
}

/// Constructs a crate containing the macro definition for the given file.
#[salsa::tracked]
fn single_file_crate(db: &dyn Db, file: VirtFile) -> Crate {
    let template = RuntimeAnalysis::get(db).template(db);
    let mut data = template.data(db).clone();
    data.root_file_id = file.file_id();

    Crate::builder(
        data,
        template.extra_data(db).clone(),
        template.workspace_data(db).clone(),
        template.cfg_options(db).clone(),
        template.env(db).clone(),
    )
    .new(db)
}

fn classify_type(ty: Ty<'_>) -> RsType {
    if ty.is_unit() {
        RsType::Void
    } else {
        match ty.as_builtin() {
            Some(BuiltinType::Int(BuiltinInt::I8)) => RsType::I8,
            Some(BuiltinType::Int(BuiltinInt::I16)) => RsType::I16,
            Some(BuiltinType::Int(BuiltinInt::I32)) => RsType::I32,
            Some(BuiltinType::Uint(BuiltinUint::U8)) => RsType::U8,
            Some(BuiltinType::Uint(BuiltinUint::U16)) => RsType::U16,
            Some(BuiltinType::Uint(BuiltinUint::U32)) => RsType::U32,
            Some(BuiltinType::Float(BuiltinFloat::F32)) => RsType::F32,
            Some(BuiltinType::Float(BuiltinFloat::F64)) => RsType::F64,
            _ => RsType::JsValue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::RootDatabase;
    use crate::test_common::{database, template};

    #[test]
    fn numeric_signatures() {
        let db = database();

        ra_ap_hir::attach_db(&db, || {
            for (name, expected) in [
                ("i8", RsType::I8),
                ("i16", RsType::I16),
                ("i32", RsType::I32),
                ("u8", RsType::U8),
                ("u16", RsType::U16),
                ("u32", RsType::U32),
                ("f32", RsType::F32),
                ("f64", RsType::F64),
            ] {
                let macro_ = RsMacro::new(
                    &db,
                    vec![
                        RsPart::Text(format!(
                            "type Number = {name}; fn identity(value: Number) -> Number {{ value }} identity("
                        )),
                        RsPart::Input,
                        RsPart::Text(")".into()),
                    ],
                );

                assert_eq!(macro_.inputs(&db), &[expected], "{name}");
                assert_eq!(macro_.ret_type(&db), expected, "{name}");
                assert_eq!(
                    expected.transport(),
                    crate::ir::transport::TransportTypeRs::F64
                );
                assert_eq!(expected.js(), crate::ir::runtime::JsType::Number);
            }

            for (body, expected) in [
                ("42", RsType::I32),
                ("1.5", RsType::F64),
                ("42u64", RsType::JsValue),
            ] {
                let macro_ = RsMacro::new(&db, vec![RsPart::Text(body.into())]);
                assert_eq!(macro_.ret_type(&db), expected, "{body}");
            }
        });
    }

    #[test]
    fn signatures_need_only_shared_database_access() {
        let db = database();

        ra_ap_hir::attach_db(&db, || {
            for (body, expected) in [
                ("", RsType::Void),
                ("()", RsType::Void),
                ("return ();", RsType::Void),
                ("fn nothing() {} nothing()", RsType::Void),
                ("__rustier_env.undefined()", RsType::JsValue),
                ("__rustier_env.undefined().discard()", RsType::Void),
            ] {
                let macro_ = RsMacro::new(&db, vec![RsPart::Text(body.into())]);
                assert_eq!(macro_.ret_type(&db), expected, "{body}");
            }
            let macro_ = RsMacro::new(
                &db,
                vec![
                    RsPart::Text("let _: () = ".into()),
                    RsPart::Input,
                    RsPart::Text("; ".into()),
                    RsPart::Input,
                ],
            );
            assert_eq!(macro_.inputs(&db), &[RsType::Void, RsType::JsValue]);
            assert_eq!(macro_.ret_type(&db), RsType::JsValue);
        });
    }

    #[test]
    fn dependency_snapshot_changes_invalidate_the_same_macro() {
        let mut db = RootDatabase::default();
        let initial = template(&mut db, "pub fn value() {} pub fn consume(_: ()) {}");
        RuntimeAnalysis::set(&mut db, initial);
        let body = vec![RsPart::Text("dep::value()".into())];

        ra_ap_hir::attach_db(&db, || {
            assert_eq!(RsMacro::new(&db, body.clone()).ret_type(&db), RsType::Void);
            let input_body = vec![
                RsPart::Text("dep::consume(".into()),
                RsPart::Input,
                RsPart::Text(")".into()),
            ];
            assert_eq!(RsMacro::new(&db, input_body).inputs(&db), &[RsType::Void]);
        });

        let changed = template(&mut db, "pub fn value() -> u32 { 42 }");
        RuntimeAnalysis::set(&mut db, changed);

        ra_ap_hir::attach_db(&db, || {
            assert_eq!(RsMacro::new(&db, body).ret_type(&db), RsType::U32);
        });
    }

    #[test]
    fn snippets_are_isolated_from_each_other() {
        let db = database();

        ra_ap_hir::attach_db(&db, || {
            let first = RsMacro::new(&db, vec![RsPart::Text("fn helper() {} helper()".into())]);
            let second = RsMacro::new(
                &db,
                vec![RsPart::Text("fn helper() -> u32 { 1 } helper()".into())],
            );
            assert_eq!(first.ret_type(&db), RsType::Void);
            assert_eq!(second.ret_type(&db), RsType::U32);
            assert_eq!(first.ret_type(&db), RsType::Void);
        });
    }

    #[test]
    fn unchanged_configuration_retains_the_inference_cache() {
        let mut db = database();
        let input = RuntimeAnalysis::get(&db);
        let template = input.template(&db);
        let body = vec![RsPart::Text("()".into())];

        ra_ap_hir::attach_db(&db, || {
            assert_eq!(RsMacro::new(&db, body.clone()).ret_type(&db), RsType::Void);
        });

        let file = VirtFile::new(
            &db,
            RsMacro::new(&db, body.clone())
                .render_analysis_rs(&db, None)
                .as_str(),
        );
        let before = single_file_crate(&db, file);

        RuntimeAnalysis::set(&mut db, template);

        ra_ap_hir::attach_db(&db, || {
            assert_eq!(before, single_file_crate(&db, file));
            assert_eq!(template, input.template(&db));
            assert_eq!(RsMacro::new(&db, body).ret_type(&db), RsType::Void);
        });
    }

    #[test]
    fn concurrent_queries_do_not_mix_snippets() {
        let db = database();
        std::thread::scope(|scope| {
            for (text, expected) in [
                ("fn helper() {} helper()", RsType::Void),
                ("fn helper() -> u32 { 1 } helper()", RsType::U32),
                ("return ();", RsType::Void),
            ] {
                let db = db.clone();
                scope.spawn(move || {
                    ra_ap_hir::attach_db(&db, || {
                        let macro_ = RsMacro::new(&db, vec![RsPart::Text(text.into())]);
                        assert_eq!(macro_.ret_type(&db), expected);
                    });
                });
            }
        });
    }
}
