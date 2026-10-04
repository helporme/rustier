use miette::{Diagnostic, SourceSpan};
use ra_ap_hir::{
    Access, AsAssocItem as _, AssocItemContainer, Function as HirFunction, GenericDef,
    GenericParam as HirGenericParam, HasAttrs as _, HasSource as _,
};
use ra_ap_syntax::AstNode as _;
use thiserror::Error;

use super::{
    TypeLowerErrorUnsupported, UnstableItemError, check_stability, diagnostic_location,
    function_fqn, lower_item_span, lower_type,
};
use crate::compiler::{
    db::Db,
    diag::DiagSource,
    ir::rust::{
        Function, FunctionInherent, FunctionParent, Impl, ImplInherent, Module, Param, Receiver,
        Span, Type, TypeOrigin,
    },
};

#[salsa::tracked]
impl<'db> Function<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_parent(
        self,
        db: &'db dyn Db,
    ) -> FunctionParent<'db> {
        let hir = self.hir(db);
        let Some(item) = hir.as_assoc_item(db) else {
            return FunctionParent::Module(Module::new(db, hir.module(db)));
        };

        match item.container(db) {
            AssocItemContainer::Impl(impl_) => FunctionParent::Impl(Impl::new(db, impl_)),
            AssocItemContainer::Trait(_) => {
                unreachable!("functions from trait declarations are not lowered")
            }
        }
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_name(self, db: &'db dyn Db) -> String {
        let hir = self.hir(db);
        hir.name(db)
            .display(db, hir.module(db).krate(db).edition(db))
            .to_string()
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_receiver(self, db: &'db dyn Db) -> Option<Receiver> {
        self.hir(db)
            .self_param(db)
            .map(|receiver| match receiver.access(db) {
                Access::Shared => Receiver::Shared,
                Access::Exclusive => Receiver::Exclusive,
                Access::Owned => Receiver::Owned,
            })
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_params(self, db: &'db dyn Db) -> Vec<Param<'db>> {
        let hir = self.hir(db);
        let crate_ = hir.module(db).krate(db);

        hir.params_without_self(db)
            .into_iter()
            .enumerate()
            .map(|(index, param)| {
                let type_ = Type::new(
                    db,
                    TypeOrigin::FunctionParameter {
                        function: self,
                        index,
                    },
                );
                let name = param
                    .name(db)
                    .map(|name| name.display(db, crate_.edition(db)).to_string())
                    .unwrap_or_else(|| format!("arg{index}"));
                let span = lower_item_span(db, param);
                Param::new(db, index, name, type_, span)
            })
            .collect()
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_ret_type(
        self,
        db: &'db dyn Db,
    ) -> Option<Type<'db>> {
        let hir = self.hir(db);
        let type_ = hir.ret_type(db);
        if type_.is_unit() {
            return None;
        }
        let type_ = lower_type(
            db,
            TypeOrigin::FunctionReturn(self),
            hir.module(db).krate(db),
        )
        .expect("functions with unsupported return types are not lowered");
        Some(type_)
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_docs(self, db: &'db dyn Db) -> Option<String> {
        self.hir(db).hir_docs(db).map(|docs| docs.docs().to_owned())
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_span(self, db: &'db dyn Db) -> Option<Span> {
        lower_item_span(db, self.hir(db))
    }
}

pub(super) fn lower_function<'db>(
    db: &'db dyn Db,
    hir: HirFunction,
) -> Result<Function<'db>, Box<FunctionLowerError>> {
    let unstable = check_stability(db, hir, hir.module(db), || lower_item_span(db, hir))
        .err()
        .or_else(|| {
            GenericDef::Function(hir)
                .params(db)
                .into_iter()
                .find_map(|param| match param {
                    HirGenericParam::TypeParam(param)
                        if param.is_unstable(db) && param.default(db).is_none() =>
                    {
                        check_stability(db, param, hir.module(db), || lower_item_span(db, hir))
                            .err()
                    }
                    _ => None,
                })
        });

    if let Some(error) = unstable {
        let span = lower_item_span(db, hir);
        let (source_code, span) = diagnostic_location(db, span.as_ref());

        return Err(Box::new(FunctionLowerError {
            fqn: function_fqn(db, hir),
            source_code,
            span,
            causes: vec![FunctionLowerCause::Unstable(error)],
        }));
    }

    let mut causes = Vec::new();

    if hir.is_async(db) {
        causes.push(FunctionLowerCause::UnsupportedAsync);
    }
    if hir.is_unsafe(db) {
        causes.push(FunctionLowerCause::UnsupportedUnsafe);
    }
    if hir.is_varargs(db) {
        causes.push(FunctionLowerCause::UnsupportedVariadic);
    }
    if GenericDef::Function(hir)
        .params(db)
        .iter()
        .any(|param| matches!(param, HirGenericParam::ConstParam(_)))
    {
        causes.push(FunctionLowerCause::UnsupportedGeneric);
    }
    if let Some(source) = hir.source(db) {
        if source
            .value
            .param_list()
            .and_then(|params| params.self_param())
            .is_some_and(|receiver| receiver.colon_token().is_some())
        {
            causes.push(FunctionLowerCause::UnsupportedArbitraryReceiver);
        }
        if let Some(abi) = source.value.abi() {
            causes.push(FunctionLowerCause::UnsupportedAbi {
                abi: abi.syntax().text().to_string(),
            });
        }
    }

    check_function_substituted(db, hir, &[], &mut causes);

    if causes.is_empty() {
        Ok(Function::new(db, hir))
    } else {
        let span = lower_item_span(db, hir);
        let (source_code, span) = diagnostic_location(db, span.as_ref());

        Err(Box::new(FunctionLowerError {
            fqn: function_fqn(db, hir),
            source_code,
            span,
            causes,
        }))
    }
}

pub(super) fn lower_inherent_function<'db>(
    db: &'db dyn Db,
    hir: HirFunction,
    impl_: ImplInherent<'db>,
) -> Result<FunctionInherent<'db>, Box<FunctionLowerError>> {
    let declaration = lower_function(db, hir)?;
    let mut causes = Vec::new();

    check_function_substituted(db, hir, &impl_.partial_type_args(db), &mut causes);

    if causes.is_empty() {
        Ok(FunctionInherent::new(db, declaration, impl_))
    } else {
        let span = lower_item_span(db, hir);
        let (source_code, span) = diagnostic_location(db, span.as_ref());

        Err(Box::new(FunctionLowerError {
            fqn: function_fqn(db, hir),
            source_code,
            span,
            causes,
        }))
    }
}

fn check_function_substituted<'db>(
    db: &'db dyn Db,
    hir: HirFunction,
    type_args: &[ra_ap_hir::Type<'db>],
    causes: &mut Vec<FunctionLowerCause>,
) {
    let crate_ = hir.module(db).krate(db);
    let check_type = |type_: &ra_ap_hir::Type<'db>| super::types::check_type(db, type_, crate_);

    if let Some(receiver) = hir.self_param(db) {
        let receiver_type = super::instantiate_type(db, receiver.ty(db), type_args.iter().cloned());
        if let Err(error) = check_type(&receiver_type) {
            causes.push(FunctionLowerCause::Type(error));
        }
    }

    for (index, param) in hir.params_without_self(db).into_iter().enumerate() {
        let name = param
            .name(db)
            .map(|name| name.display(db, crate_.edition(db)).to_string())
            .unwrap_or_else(|| format!("arg{index}"));

        let parameter_type =
            super::instantiate_type(db, param.ty().clone(), type_args.iter().cloned());
        if let Err(error) = check_type(&parameter_type) {
            causes.push(FunctionLowerCause::UnsupportedParam {
                name,
                causes: vec![error],
            });
            continue;
        }
    }

    let return_type = super::instantiate_type(db, hir.ret_type(db), type_args.iter().cloned());
    if !return_type.is_unit()
        && let Err(error) = check_type(&return_type)
    {
        causes.push(FunctionLowerCause::Type(error));
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("the function \"{fqn}\" cannot be exported to JS")]
#[diagnostic(code(rust_curse::skipped_fn), severity(Warning))]
pub(super) struct FunctionLowerError {
    fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<SourceSpan>,
    #[related]
    causes: Vec<FunctionLowerCause>,
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
enum FunctionLowerCause {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Unstable(#[from] UnstableItemError),

    #[error("async functions are not supported")]
    #[diagnostic(code(rust_curse::unsupported_async_fn), severity(Warning))]
    UnsupportedAsync,
    #[error("unsafe functions are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_unsafe_fn),
        help("wrap the unsafe operations in a safe function and export that instead"),
        severity(Warning)
    )]
    UnsupportedUnsafe,
    #[error("functions with the {abi} ABI are not supported")]
    #[diagnostic(code(rust_curse::unsupported_abi), severity(Warning))]
    UnsupportedAbi { abi: String },
    #[error("C-variadic functions are not supported")]
    #[diagnostic(code(rust_curse::unsupported_variadic_fn), severity(Warning))]
    UnsupportedVariadic,
    #[error("const-generic functions are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_generic_fn),
        help(
            "export a non-generic wrapper that instantiates the generic parameters with concrete types"
        ),
        severity(Warning)
    )]
    UnsupportedGeneric,
    #[error("arbitrary self types are not supported")]
    #[diagnostic(code(rust_curse::unsupported_arbitrary_receiver), severity(Warning))]
    UnsupportedArbitraryReceiver,
    #[error("the parameter `{name}` has an unsupported type")]
    #[diagnostic(code(rust_curse::unsupported_param), severity(Warning))]
    UnsupportedParam {
        name: String,
        #[related]
        causes: Vec<TypeLowerErrorUnsupported>,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Type(#[from] TypeLowerErrorUnsupported),
}
