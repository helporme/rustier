use miette::{Diagnostic, SourceSpan};
use ra_ap_hir::{
    Access, AsAssocItem as _, AssocItemContainer, Function as HirFunction, GenericDef,
    GenericParam as HirGenericParam, HasAttrs as _, HasSource as _, HirDisplay as _,
};
use ra_ap_syntax::AstNode as _;
use thiserror::Error;

use super::{
    TypeLowerErrorUnsupported, diagnostic_location, function_fqn, is_stable, lower_item_span,
    lower_type,
    types::{contains_borrowed_values, is_clonable, is_js_type, is_string},
};
use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::rust::{
        FieldName, Function, FunctionInherent, FunctionParent, Impl, ImplInherent, ImplQualified,
        Module, Param, Receiver, Span, Trait, Type, TypeKind, TypeOrigin,
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
            AssocItemContainer::Trait(trait_) => FunctionParent::Trait(Trait::new(db, trait_)),
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
    pub(in crate::compiler::ir::rust) fn lower_ret_type(self, db: &'db dyn Db) -> Type<'db> {
        let hir = self.hir(db);

        lower_type(
            db,
            TypeOrigin::FunctionReturn(self),
            hir.module(db).krate(db),
        )
        .expect("functions with unsupported return types are not lowered")
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
    if !is_stable(db, hir, hir.module(db))
        || GenericDef::Function(hir).params(db).iter().any(|param| {
            matches!(param, HirGenericParam::TypeParam(param)
                if param.is_unstable(db) && param.default(db).is_none())
        })
    {
        return Err(Box::new(FunctionLowerError::Unstable));
    }

    let mut causes = Vec::new();

    if hir.attrs(db).is_deprecated() {
        causes.push(FunctionLowerCause::Deprecated);
    }

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

    if let Some(error) = check_function_substituted(db, hir, &[], GenericDef::Function(hir), causes)
    {
        return Err(Box::new(error));
    }

    Ok(Function::new(db, hir))
}

pub(super) fn lower_inherent_function<'db>(
    db: &'db dyn Db,
    hir: HirFunction,
    impl_: ImplInherent<'db>,
) -> Result<Option<FunctionInherent<'db>>, Box<FunctionLowerError>> {
    let declaration = lower_function(db, hir)?;

    if declaration.receiver(db).is_none() && declaration.name(db) == "prototype" {
        return Ok(None);
    }

    let shadowed = declaration.receiver(db).is_some()
        && match impl_.target(db).kind(db) {
            TypeKind::Struct(type_) => type_.fields(db).any(
                |field| matches!(field.declaration(db).name(db), FieldName::Name(name) if name == declaration.name(db)),
            ),
            TypeKind::Enum(type_) => type_
                .variants(db)
                .flat_map(|variant| variant.fields(db))
                .any(
                    |field| matches!(field.declaration(db).name(db), FieldName::Name(name) if name == declaration.name(db)),
                ),
            _ => false,
        };

    if shadowed {
        if impl_
            .declaration(db)
            .module(db)
            .path(db)
            .crate_(db)
            // FIXME: should be filtered out elsewhere, ideally by server, not here.
            .is_local(db)
        {
            let source_span = declaration.span(db);
            let (source_code, span) = diagnostic_location(db, source_span);

            MethodShadowedByField {
                fqn: function_fqn(db, hir),
                source_code,
                span,
            }
            .accumulate(db);
        }

        return Ok(None);
    }

    if let Some(error) = check_function_substituted(
        db,
        hir,
        &ImplQualified::Inherent(impl_).partial_type_args(db),
        impl_.target(db).origin(db).context(db),
        Vec::new(),
    ) {
        return Err(Box::new(error));
    }

    Ok(Some(FunctionInherent::new(db, declaration, impl_)))
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error(
    "the instance method \"{fqn}\" is not exported to JS because it is shadowed by a field with the same name"
)]
#[diagnostic(
    code(rustier::method_shadowed_by_field),
    help("rename either the field or the method to export both"),
    severity(Warning)
)]
struct MethodShadowedByField {
    fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label("shadowed instance method")]
    span: Option<SourceSpan>,
}

pub(super) fn check_function_substituted<'db>(
    db: &'db dyn Db,
    hir: HirFunction,
    type_args: &[ra_ap_hir::Type<'db>],
    context: GenericDef,
    mut causes: Vec<FunctionLowerCause>,
) -> Option<FunctionLowerError> {
    let crate_ = hir.module(db).krate(db);
    let check_type = |type_: &ra_ap_hir::Type<'db>| super::types::check_type(db, type_, crate_);
    let parameters = hir
        .self_param(db)
        .map(|receiver| ("self".to_owned(), receiver.ty(db)))
        .into_iter()
        .chain(
            hir.params_without_self(db)
                .into_iter()
                .enumerate()
                .map(|(index, param)| {
                    let name = param
                        .name(db)
                        .map(|name| name.display(db, crate_.edition(db)).to_string())
                        .unwrap_or_else(|| format!("arg{index}"));

                    (name, param.ty().clone())
                }),
        );

    for (name, type_) in parameters {
        let parameter_type = super::instantiate_type(db, type_, type_args.iter().cloned(), context);

        if is_js_type(db, &parameter_type, "JsEnv") {
            continue;
        }

        match check_type(&parameter_type) {
            Ok(())
                if parameter_type.as_adt().is_some()
                    && !is_string(db, &parameter_type, crate_)
                    && is_clonable(db, &parameter_type, crate_) == Some(false) =>
            {
                causes.push(FunctionLowerCause::NonClonableMovedParam {
                    name,
                    type_fqn: parameter_type
                        .display(db, crate_.to_display_target(db))
                        .to_string(),
                });
            }
            Ok(()) => {}
            Err(TypeLowerErrorUnsupported::Unstable) => return Some(FunctionLowerError::Unstable),
            Err(TypeLowerErrorUnsupported::BorrowedValues { .. }) => {
                causes.push(FunctionLowerCause::BorrowedParam {
                    name,
                    type_fqn: parameter_type
                        .display(db, crate_.to_display_target(db))
                        .to_string(),
                });
            }
            Err(error) => {
                causes.push(FunctionLowerCause::UnsupportedParam {
                    name,
                    causes: vec![error],
                });
            }
        }
    }

    let return_type =
        super::instantiate_type(db, hir.ret_type(db), type_args.iter().cloned(), context);
    match check_type(&return_type) {
        Ok(()) if contains_borrowed_values(db, &return_type) => {
            causes.push(FunctionLowerCause::BorrowedReturn);
        }
        Ok(()) => {}
        Err(TypeLowerErrorUnsupported::Unstable) => return Some(FunctionLowerError::Unstable),
        Err(TypeLowerErrorUnsupported::BorrowedValues { .. }) => {
            causes.push(FunctionLowerCause::BorrowedReturn);
        }
        Err(error) => causes.push(FunctionLowerCause::Type(error)),
    }

    if causes.is_empty() {
        return None;
    }

    let span = lower_item_span(db, hir);
    let (source_code, span) = diagnostic_location(db, span.as_ref());

    Some(FunctionLowerError::Unsupported {
        fqn: function_fqn(db, hir),
        source_code,
        span,
        causes,
    })
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub(super) enum FunctionLowerError {
    #[error("unstable items are not supported")]
    Unstable,

    #[error("the function \"{fqn}\" cannot be exported to JS")]
    #[diagnostic(code(rustier::skipped_fn), severity(Warning))]
    Unsupported {
        fqn: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<SourceSpan>,
        #[related]
        causes: Vec<FunctionLowerCause>,
    },
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub(super) enum FunctionLowerCause {
    #[error("deprecated functions and methods are not exported to JS")]
    #[diagnostic(code(rustier::deprecated_item), severity(Warning))]
    Deprecated,

    #[error("async functions are not supported")]
    #[diagnostic(code(rustier::unsupported_async_fn), severity(Warning))]
    UnsupportedAsync,
    #[error("unsafe functions are not supported")]
    #[diagnostic(
        code(rustier::unsupported_unsafe_fn),
        help("wrap the unsafe operations in a safe function and export that instead"),
        severity(Warning)
    )]
    UnsupportedUnsafe,
    #[error("functions with the {abi} ABI are not supported")]
    #[diagnostic(code(rustier::unsupported_abi), severity(Warning))]
    UnsupportedAbi { abi: String },
    #[error("C-variadic functions are not supported")]
    #[diagnostic(code(rustier::unsupported_variadic_fn), severity(Warning))]
    UnsupportedVariadic,
    #[error("const-generic functions are not supported")]
    #[diagnostic(
        code(rustier::unsupported_generic_fn),
        help(
            "export a non-generic wrapper that instantiates the generic parameters with concrete types"
        ),
        severity(Warning)
    )]
    UnsupportedGeneric,
    #[error("arbitrary self types are not supported")]
    #[diagnostic(code(rustier::unsupported_arbitrary_receiver), severity(Warning))]
    UnsupportedArbitraryReceiver,
    #[error("the parameter `{name}` has an unsupported type")]
    #[diagnostic(code(rustier::unsupported_param), severity(Warning))]
    UnsupportedParam {
        name: String,
        #[related]
        causes: Vec<TypeLowerErrorUnsupported>,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Type(#[from] TypeLowerErrorUnsupported),
    #[error("the parameter `{name}` has type \"{type_fqn}\", which contains borrowed values")]
    #[diagnostic(
        code(rustier::borrowed_param),
        help("use owned values inside the ADT; a reference to an owned ADT is supported"),
        severity(Warning)
    )]
    BorrowedParam { name: String, type_fqn: String },
    #[error("the parameter `{name}` moves \"{type_fqn}\", which does not implement `Clone`")]
    #[diagnostic(
        code(rustier::non_clonable_moved_param),
        help("pass the value by reference or implement `Clone` for the type"),
        severity(Warning)
    )]
    NonClonableMovedParam { name: String, type_fqn: String },
    #[error("functions cannot return borrowed values")]
    #[diagnostic(code(rustier::borrowed_return), severity(Warning))]
    BorrowedReturn,
}
