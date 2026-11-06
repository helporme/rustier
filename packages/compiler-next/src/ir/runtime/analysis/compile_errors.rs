//! Compile error analysis using rust-analyzer.

use std::collections::HashSet;

use miette::Diagnostic;
use ra_ap_base_db::Crate;
use ra_ap_hir::{
    Adt, AnyDiagnostic, AssocItem, HirDisplay, InFile, ModuleDef, Semantics, UnsafeLint,
};
use ra_ap_hir_def::{
    DefWithBodyId,
    expr_store::{ExpressionStore, body::Body},
    hir::Expr,
    lang_item::lang_items,
};
use ra_ap_hir_ty::{
    InferenceResult,
    lang_items::lang_items_for_bin_op,
    next_solver::{
        DbInterner, TraitRef, TypingMode,
        infer::{
            DbInternerInferExt,
            traits::{Obligation, ObligationCause},
        },
        obligation_ctxt::ObligationCtxt,
    },
    solver_errors::{SolverDiagnostic, SolverDiagnosticKind},
};
use ra_ap_syntax::{SourceFile, SyntaxNodePtr, TextRange};

use crate::db::{Db, VirtFile};

macro_rules! hir_error {
    ($diagnostic:expr, $sema:expr;
        $($variant:ident($d:ident) $(if $guard:expr)? =>
            $code:expr, $kind:ident($span:expr), $message:expr $(, help: $help:expr)?;)*
        @ignore $($ignored:ident),* $(,)?
    ) => {
        match $diagnostic {
            $(AnyDiagnostic::$variant($d) $(if $guard)? => (
                $code,
                hir_error!(@$kind $sema, $span),
                String::from($message),
                hir_error!(@help $($help)?),
            ),)*
            $(AnyDiagnostic::$ignored(_))|* => return None,
        }
    };
    (@node $sema:expr, $span:expr) => {
        $sema.diagnostics_display_range($span.map(SyntaxNodePtr::from))
    };
    (@range $sema:expr, $span:expr) => {
        $sema.diagnostics_display_range_for_range($span)
    };
    (@help $help:expr) => { $help };
    (@help) => { None };
}

#[derive(Clone, Debug, PartialEq, Eq, salsa::Update, thiserror::Error)]
#[error("{message}")]
pub struct CompileError {
    pub code: &'static str,
    pub message: String,
    pub help: Option<String>,
    pub ranges: Vec<(usize, TextRange)>,
}

impl CompileError {
    fn from_hir<'db>(
        db: &'db dyn Db,
        file: VirtFile,
        crate_: ra_ap_hir::Crate,
        diagnostic: AnyDiagnostic<'db>,
    ) -> Option<Self> {
        let sema = Semantics::new_dyn(db);
        let target = crate_.to_display_target(db);
        let edition = crate_.edition(db);
        let (code, span, message, help) = hir_error! {
            diagnostic, sema;
            TypeMismatch(d) if !d.expected.is_unknown() && !d.actual.is_unknown() =>
                "E0308", node(d.expr_or_pat), format!("expected `{}`, found `{}`", d.expected.display(db, target), d.actual.display(db, target));
            UnimplementedTrait(d) =>
                "E0277", node(d.span), format!("the trait bound `{}` is not satisfied", d.trait_predicate.display(db, target)),
                help: (!d.parent_trait_predicates.is_empty()).then(|| d.parent_trait_predicates.iter().map(|p| format!("required by the bound `{}`", p.display(db, target))).collect::<Vec<_>>().join("\n"));
            UnresolvedIdent(d) =>
                "E0425", range(d.node.map(|(ptr, range)| range.unwrap_or_else(|| SyntaxNodePtr::from(ptr).text_range()))), "cannot find this value in scope";
            UnresolvedMethodCall(d) =>
                "E0599", node(d.expr), format!("no method named `{}` found for type `{}`", d.name.display(db, edition), d.receiver.display(db, target)),
                help: d.field_with_same_name.is_some().then(|| "a field with this name exists; remove the call parentheses".into());
            UnresolvedField(d) =>
                "E0609", node(d.expr), format!("no field `{}` on type `{}`", d.name.display(db, edition), d.receiver.display(db, target)),
                help: d.method_with_same_name_exists.then(|| "a method with this name exists; use parentheses to call it".into());
            ExpectedFunction(d) => "E0618", node(d.call), format!("expected a function, found `{}`", d.found.display(db, target));
            CannotBeDereferenced(d) => "E0614", node(d.expr), format!("type `{}` cannot be dereferenced", d.found.display(db, target));
            CannotIndexInto(d) => "E0608", node(d.expr), format!("cannot index into a value of type `{}`", d.found.display(db, target));
            TypeMustBeKnown(d) => "E0282", node(d.at_point), "type annotations needed", help: Some("add an explicit Rust type annotation".into());
            TypedHole(d) => "E0121", node(d.expr), format!("expected an expression of type `{}`", d.expected.display(db, target));
            MismatchedArgCount(d) => if d.is_fn_trait_call { "E0057" } else { "E0061" }, node(d.call_expr), format!("expected {} arguments, found {}", d.expected, d.found);
            MismatchedTupleStructPatArgCount(d) => "E0023", node(d.expr_or_pat), format!("expected {} fields, found {}", d.expected, d.found);
            MismatchedArrayPatLen(d) => "E0527", node(d.pat), format!("expected {} elements, found {}", d.expected, d.found);
            MissingFields(d) => "E0063", node(InFile::new(d.file, d.field_list_parent)), format!("missing fields: {}", d.missed_fields.iter().map(|(name, _)| name.display(db, edition).to_string()).collect::<Vec<_>>().join(", "));
            MissingMatchArms(d) => "E0004", node(d.scrutinee_expr), format!("non-exhaustive match: {}", d.uncovered_patterns);
            NonExhaustiveLet(d) => "E0005", node(d.pat), format!("refutable pattern: {}", d.uncovered_patterns);
            NeedMut(d) => "E0596", node(d.span), "cannot mutate an immutable binding", help: Some("declare the binding with `mut`".into());
            MovedOutOfRef(d) => "E0507", node(d.span), format!("cannot move `{}` out of a reference", d.ty.display(db, target));
            MissingUnsafe(d) if matches!(d.lint, UnsafeLint::HardError) =>
                "E0133", node(d.node), "unsafe operation requires an unsafe block",
                help: Some("use an `unsafe` block only after verifying the operation's safety requirements".into());
            MacroError(d) if d.error => "macro-error", range(d.range), d.message;
            MacroExpansionParseError(d) => "syntax-error", range(d.range), d.errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
            MacroDefError(d) => "macro-error", node(d.node), d.message;
            UnresolvedMacroCall(d) => "unresolved-macro-call", range(d.range), "cannot resolve macro";
            UnresolvedImport(d) => "E0432", node(d.decl), "unresolved import";
            UnresolvedExternCrate(d) => "E0463", node(d.decl), "cannot find crate";
            UnresolvedModule(d) => "E0583", node(d.decl), "file not found for module";
            UnresolvedAssocItem(d) => "E0599", node(d.expr_or_pat), "associated item not found";
            PrivateAssocItem(d) => "E0624", node(d.expr_or_pat), "associated item is private";
            PrivateField(d) => "E0616", node(d.expr), "field is private";
            NoSuchField(d) => "E0559", node(d.field), "unknown field";
            DuplicateField(d) => "E0062", node(d.field), "field specified more than once";
            AwaitOutsideOfAsync(d) => "E0728", node(d.node), "await is only allowed inside async functions and blocks";
            BreakOutsideOfLoop(d) => "E0268", node(d.expr), "invalid break or continue expression";
            UndeclaredLabel(d) => "E0426", node(d.node), "undeclared label";
            UnreachableLabel(d) => "E0767", node(d.node), "unreachable label";
            YieldOutsideCoroutine(d) => "E0627", node(d.expr), "yield outside of a coroutine";
            ReturnOutsideFunction(d) => "E0572", node(d.expr), "return outside of a function";
            InvalidLhsOfAssignment(d) => "E0070", node(d.lhs), "invalid left-hand side of assignment";
            InvalidCast(d) => "E0606", node(d.expr), format!("invalid cast from `{}` to `{}`", d.expr_ty.display(db, target), d.cast_ty.display(db, target));
            CastToUnsized(d) => "E0620", node(d.expr), "cannot cast to an unsized type";
            InvalidDeriveTarget(d) => "E0774", range(d.range), "derive may only be applied to structs, enums and unions";
            MalformedDerive(d) => "E0777", range(d.range), "malformed derive attribute";
            ArrayPatternWithoutFixedLength(d) => "E0730", node(d.pat), "cannot pattern-match an array without a fixed length";
            ExpectedArrayOrSlicePat(d) => "E0529", node(d.pat), format!("expected an array or slice, found `{}`", d.found.display(db, target));
            InvalidRangePatType(d) => "E0029", node(d.pat), "invalid type in range pattern";
            MutRefInImmRefPat(d) => "E0308", node(d.pat), "mutable reference pattern in an immutable reference";
            CannotImplicitlyDerefTraitObject(d) => "E0033", node(d.pat), "cannot implicitly dereference a trait object";
            FruInDestructuringAssignment(d) => "E0797", node(d.node), "functional record update is not allowed in destructuring assignment";
            FunctionalRecordUpdateOnNonStruct(d) => "E0436", node(d.base_expr), "functional record update requires a struct";
            NonExhaustiveRecordExpr(d) => "E0639", node(d.expr), "cannot construct a non-exhaustive struct";
            NonExhaustiveRecordPat(d) => "E0638", node(d.pat), "non-exhaustive struct pattern requires `..`";
            IncoherentImpl(d) => "E0116", node(InFile::new(d.file_id, d.impl_)), "cannot define an inherent impl for a type outside its crate";
            TraitImplOrphan(d) => "E0117", node(InFile::new(d.file_id, d.impl_)), "trait implementation violates the orphan rules";
            TraitImplIncorrectSafety(d) => "trait-impl-safety", node(InFile::new(d.file_id, d.impl_)), if d.should_be_safe { "trait implementation must not be unsafe" } else { "trait implementation must be unsafe" };
            TraitImplMissingAssocItems(d) => "E0046", node(InFile::new(d.file_id, d.impl_)), format!("missing trait items: {}", d.missing.iter().map(|(name, _)| name.display(db, edition).to_string()).collect::<Vec<_>>().join(", "));
            TraitImplRedundantAssocItems(d) => "E0407", node(InFile::new(d.file_id, d.impl_)), "item is not a member of the trait";
            GenericArgsProhibited(d) => "E0109", node(d.args), "generic arguments are not allowed here";
            ParenthesizedGenericArgsWithoutFnTrait(d) => "E0214", node(d.args), "parenthesized generic arguments require a function trait";
            BadRtn(d) => "bad-return-type-notation", node(d.rtn), "invalid return type notation";
            InferVarsNotAllowed(d) => "E0121", node(d.node), "inferred types are not allowed here";
            IncorrectGenericsLen(d) => "E0107", node(d.generics_or_segment), format!("expected {} generic arguments, found {}", d.expected, d.provided);
            MissingLifetime(d) => "E0106", node(d.generics_or_segment), "missing lifetime specifier";
            ElidedLifetimesInPath(d) if d.hard_error => "E0726", node(d.generics_or_segment), "implicit elided lifetime not allowed here";
            IncorrectGenericsOrder(d) => "E0747", node(d.provided_arg), "generic arguments are in the wrong order";
            GenericDefaultRefersToSelf(d) => "E0735", node(d.segment), "generic default cannot refer to `Self`";
            UnionExprMustHaveExactlyOneField(d) => "E0784", node(d.expr), "union construction requires exactly one field";
            UnionPatMustHaveExactlyOneField(d) => "union-pattern", node(d.pat), "union pattern requires exactly one field";
            UnionPatHasRest(d) => "union-pattern", node(d.pat), "union pattern cannot contain `..`";
            MethodCallIllegalSizedBound(d) => "sized-bound", node(d.call_expr), "method cannot be invoked because its Sized bound is not satisfied";
            PatternArgInExternFn(d) => "E0130", node(d.node), "patterns are not allowed in foreign function declarations";
            MutableRefBinding(d) => "mutable-ref-binding", node(d.pat), "invalid mutable reference binding";
            ExplicitDropMethodUse(d) => "E0040", node(d.expr_or_path.either(|s| s.map(SyntaxNodePtr::from), |s| s.map(SyntaxNodePtr::from))), "explicit destructor calls are not allowed", help: Some("use `drop(value)` instead".into());
            @ignore TypeMismatch, MissingUnsafe, MacroError, ElidedLifetimesInPath,
                InactiveCode, IncorrectCase, RemoveTrailingReturn, RemoveUnnecessaryElse,
                UnusedMustUse, ReplaceFilterMapNextWithFindMap, UnusedMut, UnusedVariable,
                UnimplementedBuiltinMacro
        };

        Some(Self {
            code,
            message,
            help,
            ranges: map_range(
                db,
                file,
                (span.file_id == file.file_id()).then_some(span.range),
            ),
        })
    }
}

impl Diagnostic for CompileError {
    fn code(&self) -> Option<Box<dyn std::fmt::Display + '_>> {
        Some(Box::new(self.code))
    }

    fn severity(&self) -> Option<miette::Severity> {
        Some(miette::Severity::Error)
    }

    fn help(&self) -> Option<Box<dyn std::fmt::Display + '_>> {
        self.help
            .as_ref()
            .map(|help| Box::new(help) as Box<dyn std::fmt::Display>)
    }
}

pub(super) fn check(db: &dyn Db, crate_: Crate) -> Option<Vec<CompileError>> {
    let mut errors = check_known_errors(db, crate_);

    if errors.is_empty()
        && let Some(error) = check_unknown_error(db, crate_)
    {
        errors.push(error);
    }

    (!errors.is_empty()).then_some(errors)
}

#[salsa::tracked]
fn check_known_errors(db: &dyn Db, crate_: Crate) -> Vec<CompileError> {
    let file_id = crate_.data(db).root_file_id;
    let file = VirtFile::try_from(file_id).unwrap();
    let source = db.file_text(file_id);
    let crate_: ra_ap_hir::Crate = crate_.into();
    let parsed = SourceFile::parse(source.text(db), crate_.edition(db));
    let diagnostics = if !parsed.errors().is_empty() {
        parsed
            .errors()
            .iter()
            .map(|error| CompileError {
                code: "syntax-error",
                message: error.to_string(),
                help: None,
                ranges: map_range(db, file, Some(error.range())),
            })
            .collect::<Vec<_>>()
    } else {
        let mut diagnostics = Vec::new();
        crate_
            .root_module(db)
            .diagnostics(db, &mut diagnostics, false);

        diagnostics
            .into_iter()
            .filter_map(|diagnostic| CompileError::from_hir(db, file, crate_, diagnostic))
            .collect()
    };

    let mut errors = Vec::new();
    for error in diagnostics {
        if !errors.contains(&error) {
            errors.push(error);
        }
    }

    errors
}

#[salsa::tracked]
fn check_unknown_error(db: &dyn Db, crate_: Crate) -> Option<CompileError> {
    let mut modules = vec![ra_ap_hir::Crate::from(crate_).root_module(db)];
    let mut seen = HashSet::new();

    while let Some(module) = modules.pop() {
        let mut definitions = module.declarations(db);
        let mut associated = module
            .impl_defs(db)
            .into_iter()
            .flat_map(|impl_| impl_.items(db))
            .collect::<Vec<_>>();

        while let Some(definition) = definitions.pop().or_else(|| {
            associated.pop().map(|item| match item {
                AssocItem::Function(f) => ModuleDef::Function(f),
                AssocItem::Const(c) => ModuleDef::Const(c),
                AssocItem::TypeAlias(t) => ModuleDef::TypeAlias(t),
            })
        }) {
            match definition {
                ModuleDef::Module(module) => modules.push(module),
                ModuleDef::Trait(trait_) => associated.extend(trait_.items(db)),
                ModuleDef::Adt(Adt::Enum(enum_)) => {
                    definitions.extend(enum_.variants(db).into_iter().map(ModuleDef::EnumVariant))
                }
                _ => {}
            }
            let Some(owner) = definition.as_def_with_body() else {
                continue;
            };
            let Ok(owner) = DefWithBodyId::try_from(owner) else {
                continue;
            };
            if !seen.insert(owner) {
                continue;
            }

            let inference = InferenceResult::of(db, owner);
            if inference
                .expression_types()
                .map(|(_, ty)| ty)
                .chain(inference.pattern_types().map(|(_, ty)| ty))
                .chain(inference.binding_types().map(|(_, ty)| ty))
                .any(|ty| ty.references_non_lt_error())
            {
                if let Some(error) = explain_inference_error(db, crate_, owner, inference) {
                    return Some(error);
                }

                return Some(CompileError {
                    code: "rustier::unknown",
                    message: "The macro has a type inference error".into(),
                    help: Some(
                        "rust-analyzer did not provide a detailed diagnostic for this error".into(),
                    ),
                    ranges: Vec::new(),
                });
            }

            modules.extend(
                Body::of(db, owner)
                    .blocks(db)
                    .map(|(_, map)| ra_ap_hir::Module::from(map.root_module_id())),
            );
        }
    }

    None
}

fn explain_inference_error<'db>(
    db: &'db dyn Db,
    crate_: Crate,
    owner: DefWithBodyId,
    inference: &InferenceResult<'db>,
) -> Option<CompileError> {
    let interner = DbInterner::new_with(db, crate_);
    let lang_items = lang_items(db, crate_);
    let param_env = db.trait_environment(owner.generic_def(db));
    let (store, source_map) = ExpressionStore::with_source_map(db, owner.into());
    let file = VirtFile::try_from(crate_.data(db).root_file_id).unwrap();
    let target = ra_ap_hir::Crate::from(crate_).to_display_target(db);
    let sema = Semantics::new_dyn(db);

    for (expr, ty) in inference.expression_types() {
        if !ty.references_non_lt_error() {
            continue;
        }
        let Expr::BinaryOp {
            lhs,
            rhs,
            op: Some(op),
        } = store[expr]
        else {
            continue;
        };
        let Some((_, Some(trait_))) = lang_items_for_bin_op(&lang_items, op) else {
            continue;
        };
        let operands = [inference.expr_ty(lhs), inference.expr_ty(rhs)];
        if operands
            .iter()
            .any(|ty| ty.references_non_lt_error() || ty.is_infer())
        {
            continue;
        }

        let infcx = interner.infer_ctxt().build(TypingMode::PostAnalysis);
        let mut obligations = ObligationCtxt::new(&infcx);
        obligations.register_obligation(Obligation::new(
            interner,
            ObligationCause::new(expr),
            param_env,
            TraitRef::new(interner, trait_.into(), operands),
        ));

        for error in obligations.evaluate_obligations_error_on_ambiguity() {
            let error = error.into_fulfillment_error(&infcx);
            let Some(diagnostic) = SolverDiagnostic::from_fulfillment_error(&error) else {
                continue;
            };
            let SolverDiagnosticKind::TraitUnimplemented {
                trait_predicate,
                parent_trait_predicates,
            } = diagnostic.kind;
            let span = source_map.expr_syntax(expr).ok()?;
            let span = sema.diagnostics_display_range(span.map(SyntaxNodePtr::from));

            return Some(CompileError {
                code: "E0277",
                message: format!(
                    "the trait bound `{}` is not satisfied",
                    trait_predicate.get(interner).display(db, target)
                ),
                help: (!parent_trait_predicates.is_empty()).then(|| {
                    parent_trait_predicates
                        .iter()
                        .map(|predicate| {
                            format!(
                                "required by the bound `{}`",
                                predicate.get(interner).display(db, target)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }),
                ranges: map_range(
                    db,
                    file,
                    (span.file_id == file.file_id()).then_some(span.range),
                ),
            });
        }
    }

    None
}

fn map_range(db: &dyn Db, file: VirtFile, range: Option<TextRange>) -> Vec<(usize, TextRange)> {
    let Some(range) = range else {
        return Vec::new();
    };

    file.source_map(db)
        .iter()
        .filter_map(|(generated, part)| {
            let lo = range.start().max(generated.start());
            let hi = range.end().min(generated.end());
            if lo >= hi && !(range.is_empty() && generated.contains(range.start())) {
                return None;
            }

            Some((
                *part,
                TextRange::new(lo - generated.start(), hi - generated.start()),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use salsa::Setter;

    use crate::{
        codegen::{TransportBackend, TransportBackendNode, TransportBackendWasm, WasmImport},
        diag::Theme,
        ir::{
            js::{JsFile, JsProgram},
            runtime::{RsMacro, RsPart, RuntimeAnalysis},
        },
        test_common::{database, template},
    };

    #[test]
    fn checks_syntax_types_resolution_and_bridge_return_types() {
        let db = database();
        ra_ap_hir::attach_db(&db, || {
            for (body, code) in [
                ("let = ;", "syntax-error"),
                ("let _: bool = 3; ()", "E0308"),
                ("missing()", "E0425"),
                ("macro_rules! bad { () => { missing } } bad!()", "E0425"),
                ("__rustier_env.missing()", "E0599"),
                ("fn helper() { let _: bool = 3; } helper()", "E0308"),
                ("\"borrowed\"", "E0308"),
                ("&(1, 2)", "E0308"),
            ] {
                let macro_ = RsMacro::new(&db, vec![RsPart::Text(body.into())]);
                let errors = macro_.check_compile_errors(&db).as_ref().expect(body);

                assert!(
                    errors.iter().any(|error| error.code == code),
                    "{body}: {errors:?}"
                );
                assert!(!macro_.is_compilable(&db));
            }

            for body in [
                "()",
                "true",
                "42",
                "42u64",
                "(1, 2)",
                "let unused = 1; ()",
                "__rustier_env.null()",
            ] {
                assert!(
                    RsMacro::new(&db, vec![RsPart::Text(body.into())]).is_compilable(&db),
                    "{body}"
                );
            }
        });
    }

    #[test]
    fn errors_are_mapped_per_occurrence_in_utf8_bytes() {
        let db = database();
        ra_ap_hir::attach_db(&db, || {
            let first = JsFile::new(
                &db,
                "/first.ts",
                "import { rs } from 'rustier'; const café = rs`missing`; rs`missing`;",
            );
            let second = JsFile::new(
                &db,
                "/second.ts",
                "import { rs } from 'rustier';\nrs`missing`;",
            );
            let program = JsProgram::new(&db, vec![first, second]);
            let diagnostics = program.check(&db);

            assert_eq!(program.runtime(&db).macros(&db).len(), 1);
            assert_eq!(diagnostics.len(), 3);
            for (diagnostic, (file, offset)) in diagnostics.iter().zip([
                (first, first.contents(&db).find("missing").unwrap()),
                (first, first.contents(&db).rfind("missing").unwrap()),
                (second, second.contents(&db).find("missing").unwrap()),
            ]) {
                let labels: Vec<_> = diagnostic.inner.labels().unwrap().collect();
                let rendered = diagnostic.render(Theme::UnicodeNoColor).unwrap();

                assert_eq!(labels.len(), 1);
                assert_eq!(labels[0].offset(), offset);
                assert_eq!(labels[0].len(), "missing".len());
                assert!(rendered.contains(file.module_id(&db)));
                assert!(!rendered.contains("__rustier_arg"));
            }
            assert!(diagnostics == program.check(&db));
        });
    }

    #[test]
    fn unknown_inference_error_labels_the_macro() {
        let db = database();
        ra_ap_hir::attach_db(&db, || {
            let source = "import { rs } from 'rustier'; const café = true;\nrs`${café} + 3`;";
            let file = JsFile::new(&db, "/input.ts", source);
            let diagnostics = JsProgram::new(&db, vec![file]).check(&db);

            assert_eq!(diagnostics.len(), 1);
            let labels: Vec<_> = diagnostics[0].inner.labels().unwrap().collect();
            let rendered = diagnostics[0].render(Theme::UnicodeNoColor).unwrap();

            assert_eq!(labels.len(), 1);
            assert_eq!(
                &source[labels[0].offset()..labels[0].offset() + labels[0].len()],
                "rs`${café} + 3`"
            );
            assert_eq!(
                diagnostics[0].inner.code().unwrap().to_string(),
                "rustier::unknown"
            );
            assert!(rendered.contains("The macro has a type inference error"));
            assert!(rendered.contains("rust-analyzer did not provide a detailed diagnostic"));
            assert!(!rendered.contains("__rustier_arg"));
        });
    }

    #[test]
    fn parsing_and_compile_diagnostics_are_both_accumulated() {
        let db = database();
        ra_ap_hir::attach_db(&db, || {
            let parse_error = JsFile::new(&db, "/parse.ts", "const = ;");
            let rust_error = JsFile::new(
                &db,
                "/rust.ts",
                "import { rs } from 'rustier'; rs`unknown`;",
            );
            let program = JsProgram::new(&db, vec![parse_error, rust_error]);
            let messages = program
                .check(&db)
                .iter()
                .map(|d| d.render(Theme::UnicodeNoColor).unwrap())
                .collect::<Vec<_>>()
                .join("\n");

            assert!(messages.contains("/parse.ts"));
            assert!(messages.contains("/rust.ts"));
        });
    }

    #[test]
    fn source_and_dependency_changes_invalidate_checks() {
        let mut db = database();
        let file = JsFile::new(
            &db,
            "/input.ts",
            "import { rs } from 'rustier'; rs`missing`;",
        );
        let program = JsProgram::new(&db, vec![file]);
        ra_ap_hir::attach_db(&db, || assert_eq!(program.check(&db).len(), 1));

        let fixed = JsFile::new(&db, "/input.ts", "import { rs } from 'rustier'; rs`()`;");
        program.set_files(&mut db).to(vec![fixed]);
        ra_ap_hir::attach_db(&db, || assert!(program.check(&db).is_empty()));

        let body = vec![RsPart::Text("dep::value()".into())];
        let initial = template(&mut db, "pub fn value() -> &'static str { \"borrowed\" }");
        RuntimeAnalysis::set(&mut db, initial);
        ra_ap_hir::attach_db(&db, || {
            assert!(!RsMacro::new(&db, body.clone()).is_compilable(&db))
        });

        let fixed = template(&mut db, "pub fn value() -> u32 { 42 }");
        RuntimeAnalysis::set(&mut db, fixed);
        ra_ap_hir::attach_db(&db, || assert!(RsMacro::new(&db, body).is_compilable(&db)));
    }

    #[test]
    fn invalid_macros_are_absent_from_both_bridges_and_throw_in_js() {
        let db = database();
        ra_ap_hir::attach_db(&db, || {
            let file = JsFile::new(
                &db,
                "/input.mjs",
                r#"import { rs } from 'rustier';
export const valid = () => rs`true`;
export const invalid = () => rs`let _: bool = "a\"b\\c";`;
export const nested = () => rs`${rs`missing`}`;
"#,
            );
            let program = JsProgram::new(&db, vec![file]);
            let runtime = program.runtime(&db);
            let transformed = file
                .module(&db)
                .render_transformed(&db, "./runtime.mjs".into());
            let allocator = oxc_allocator::Allocator::default();

            assert_eq!(program.check(&db).len(), 2);
            assert_eq!(transformed.matches("throw new Error(").count(), 2);
            assert!(
                oxc_parser::Parser::new(&allocator, transformed, oxc_span::SourceType::mjs())
                    .parse()
                    .diagnostics
                    .is_empty()
            );
            for backend in [
                TransportBackend::Node(TransportBackendNode),
                TransportBackend::Wasm(TransportBackendWasm {
                    mem64: false,
                    import: WasmImport::Fetch,
                }),
            ] {
                let rust = runtime.render_rs_ffi(&db, &backend);
                let js = runtime.render_js_ffi(&db, &backend, &"./bridge".into());
                syn::parse_file(&rust).unwrap();

                for macro_ in runtime.macros(&db) {
                    let valid = macro_.is_compilable(&db);
                    assert_eq!(rust.contains(macro_.symbol(&db)), valid);
                    assert_eq!(js.contains(macro_.symbol(&db)), valid);
                }
            }
        });
    }

    #[test]
    fn concurrent_check_queries_remain_isolated() {
        let db = database();
        std::thread::scope(|scope| {
            for body in ["missing", "()", "let _: bool = 42;"] {
                let db = db.clone();
                scope.spawn(move || {
                    ra_ap_hir::attach_db(&db, || {
                        let macro_ = RsMacro::new(&db, vec![RsPart::Text(body.into())]);
                        assert_eq!(macro_.is_compilable(&db), body == "()");
                    })
                });
            }
        });
    }
}
