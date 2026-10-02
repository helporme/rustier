use miette::{Diagnostic, SourceSpan};
use ra_ap_hir::{
    Access, AsAssocItem as _, AssocItemContainer, GenericDef, HasAttrs as _, HasSource as _,
};
use ra_ap_syntax::{AstNode as _, ast};
use thiserror::Error;

use super::{
    TypeLowerErrorUnsupported, diagnostic_location, function_fqn, is_clonable, lower_item_span,
    lower_type,
};
use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::rust::{
        AsItem as _, Callable, Function, FunctionInherent, FunctionParent, FunctionQualified, Impl,
        ImplInherent, Module, Param, Receiver, Span, Type, TypeKind, TypeOrigin,
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
        match type_.kind(db) {
            TypeKind::Borrowed(_) => {
                unreachable!("functions returning borrowed values are not lowered")
            }
            _ => Some(type_),
        }
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
    hir: ra_ap_hir::Function,
) -> Result<Function<'db>, Box<FunctionLowerError>> {
    lower_function_with_context(db, hir, None)
}

pub(super) fn lower_inherent_function<'db>(
    db: &'db dyn Db,
    hir: ra_ap_hir::Function,
    impl_: ImplInherent<'db>,
) -> Result<FunctionInherent<'db>, Box<FunctionLowerError>> {
    // Provisional handles provide origins for substituted signature types.
    // Only return the qualified function after its signature has been lowered.
    let function = FunctionInherent::new(db, Function::new(db, hir), impl_);
    let callable = FunctionQualified::Inherent(function)
        .as_callible(db)
        .filter(|callable| callable.needs_substitution(db));
    lower_function_with_context(db, hir, callable)?;
    Ok(function)
}

fn lower_function_with_context<'db>(
    db: &'db dyn Db,
    hir: ra_ap_hir::Function,
    callable: Option<Callable<'db>>,
) -> Result<Function<'db>, Box<FunctionLowerError>> {
    let crate_ = hir.module(db).krate(db);
    let mut causes = Vec::new();
    let lower_signature_type = |origin| {
        if callable.is_some() {
            super::types::lower_instantiated_type(db, origin, crate_)
        } else {
            lower_type(db, origin, crate_)
        }
    };

    if hir.is_async(db) {
        causes.push(FunctionLowerCause::UnsupportedAsync);
    }
    if hir.is_unsafe(db) {
        causes.push(FunctionLowerCause::UnsupportedUnsafe);
    }
    if hir.is_varargs(db) {
        causes.push(FunctionLowerCause::UnsupportedVariadic);
    }
    if !GenericDef::Function(hir).params(db).is_empty() {
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
        if source
            .value
            .syntax()
            .descendants()
            .filter_map(ast::RefType::cast)
            .any(|reference| reference.lifetime().is_some())
        {
            causes.push(FunctionLowerCause::UnsupportedLifetimeRef);
        }
    }

    if let Some(callable) = callable
        && hir.self_param(db).is_some()
        && let Err(error) = lower_signature_type(TypeOrigin::CallableReceiver(callable))
    {
        causes.push(FunctionLowerCause::Type(error));
    }

    if let Some(receiver) = hir.self_param(db)
        && receiver.access(db) == Access::Owned
    {
        let origin = callable
            .map(TypeOrigin::CallableReceiver)
            .unwrap_or_else(|| TypeOrigin::FunctionReceiver(Function::new(db, hir)));
        let receiver_type = origin.hir(db);
        match lower_signature_type(origin).map(|type_| type_.kind(db)) {
            Ok(TypeKind::Struct(struct_)) if !is_clonable(db, &receiver_type, crate_) => {
                causes.push(FunctionLowerCause::NonClonableMovedParam {
                    name: "self".to_owned(),
                    type_fqn: struct_.fqn(db).to_owned(),
                });
            }
            Ok(TypeKind::Enum(enum_)) if !is_clonable(db, &receiver_type, crate_) => {
                causes.push(FunctionLowerCause::NonClonableMovedParam {
                    name: "self".to_owned(),
                    type_fqn: enum_.fqn(db).to_owned(),
                });
            }
            _ => {}
        }
    }

    for (index, param) in hir.params_without_self(db).into_iter().enumerate() {
        let name = param
            .name(db)
            .map(|name| name.display(db, crate_.edition(db)).to_string())
            .unwrap_or_else(|| format!("arg{index}"));
        let origin = match callable {
            Some(callable) => TypeOrigin::CallableParameter { callable, index },
            None => TypeOrigin::FunctionParameter {
                function: Function::new(db, hir),
                index,
            },
        };
        let parameter_type = origin.hir(db);
        match lower_signature_type(origin).map(|type_| type_.kind(db)) {
            Err(error) => causes.push(FunctionLowerCause::UnsupportedParam {
                name,
                causes: vec![error],
            }),
            Ok(TypeKind::Struct(struct_)) if !is_clonable(db, &parameter_type, crate_) => {
                causes.push(FunctionLowerCause::NonClonableMovedParam {
                    name,
                    type_fqn: struct_.fqn(db).to_owned(),
                });
            }
            Ok(TypeKind::Enum(enum_)) if !is_clonable(db, &parameter_type, crate_) => {
                causes.push(FunctionLowerCause::NonClonableMovedParam {
                    name,
                    type_fqn: enum_.fqn(db).to_owned(),
                });
            }
            Ok(TypeKind::Borrowed(reference))
                if reference.is_mutable(db)
                    && matches!(
                        reference.inner(db).kind(db),
                        TypeKind::bool
                            | TypeKind::char
                            | TypeKind::i8
                            | TypeKind::u8
                            | TypeKind::i16
                            | TypeKind::u16
                            | TypeKind::i32
                            | TypeKind::u32
                            | TypeKind::f32
                            | TypeKind::f64
                            | TypeKind::Str
                            | TypeKind::String
                    ) =>
            {
                let span = lower_item_span(db, hir);
                let (source_code, span) = diagnostic_location(db, span.as_ref());
                MutableReferenceIgnored {
                    parameter: name,
                    source_code,
                    span,
                }
                .accumulate(db);
            }
            Ok(_) => {}
        }
    }

    let return_origin = callable
        .map(TypeOrigin::CallableReturn)
        .unwrap_or_else(|| TypeOrigin::FunctionReturn(Function::new(db, hir)));
    let return_type = return_origin.hir(db);
    if !return_type.is_unit() {
        match lower_signature_type(return_origin).map(|type_| type_.kind(db)) {
            Ok(TypeKind::Borrowed(_)) => causes.push(FunctionLowerCause::BorrowedReturn),
            Err(error) => causes.push(FunctionLowerCause::Type(error)),
            Ok(_) => {}
        }
    }

    if causes.is_empty() {
        Ok(Function::new(db, hir))
    } else {
        let span = lower_item_span(db, hir);
        let (source_code, span) = diagnostic_location(db, span.as_ref());
        Err(Box::new(FunctionLowerError {
            fqn: callable
                .map(|callable| callable.fqn(db).to_owned())
                .unwrap_or_else(|| function_fqn(db, hir)),
            source_code,
            span,
            causes,
        }))
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
    #[error("generic functions are not supported")]
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
    #[error("references with explicit lifetimes are not supported")]
    #[diagnostic(code(rust_curse::unsupported_lifetime_ref), severity(Warning))]
    UnsupportedLifetimeRef,
    #[error("the parameter `{name}` has an unsupported type")]
    #[diagnostic(code(rust_curse::unsupported_param), severity(Warning))]
    UnsupportedParam {
        name: String,
        #[related]
        causes: Vec<TypeLowerErrorUnsupported>,
    },
    #[error("the parameter `{name}` moves \"{type_fqn}\", which does not implement `Clone`")]
    #[diagnostic(
        code(rust_curse::non_clonable_moved_param),
        help("pass the value by reference or implement `Clone` for the type"),
        severity(Warning)
    )]
    NonClonableMovedParam { name: String, type_fqn: String },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Type(#[from] TypeLowerErrorUnsupported),
    #[error("functions cannot return borrowed values")]
    #[diagnostic(
        code(rust_curse::borrowed_return),
        help("return an owned value instead, for example via `.to_owned()` or `.clone()`"),
        severity(Warning)
    )]
    BorrowedReturn,
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("writes through the mutable reference parameter `{parameter}` are not observable by JS")]
#[diagnostic(
    code(rust_curse::mutable_reference_ignored),
    help("pass the value by value or return the updated value instead"),
    severity(Warning)
)]
struct MutableReferenceIgnored {
    parameter: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<SourceSpan>,
}
