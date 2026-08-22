use miette::Diagnostic;
use ra_ap_hir::{GenericDef, HasAttrs as _, HasSource as _};
use ra_ap_syntax::{AstNode as _, ast};
use thiserror::Error;

use super::{
    TypeLowerErrorUnsupported, diagnostic_location, function_fqn, is_clonable, lower_item_span,
    lower_type,
};
use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource, DiagSpan},
    ir::rust::{AsItem as _, Function, Mod, Param, Span, Type, ValueType},
};

#[salsa::tracked]
impl<'db> Function<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_parent(self, db: &'db dyn Db) -> Mod<'db> {
        Mod::new(db, self.hir(db).module(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_name(self, db: &'db dyn Db) -> String {
        let hir = self.hir(db);
        hir.name(db)
            .display(db, hir.module(db).krate(db).edition(db))
            .to_string()
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_params(self, db: &'db dyn Db) -> Vec<Param<'db>> {
        let hir = self.hir(db);
        let crate_ = hir.module(db).krate(db);
        hir.params_without_self(db)
            .into_iter()
            .enumerate()
            .map(|(index, param)| {
                let type_ = lower_type(db, param.ty(), crate_)
                    .expect("functions with unsupported parameters are not lowered");
                let name = param
                    .name(db)
                    .map(|name| name.display(db, crate_.edition(db)).to_string())
                    .unwrap_or_else(|| format!("arg{index}"));
                Param::new(db, self, index, name, type_)
            })
            .collect()
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_ret_type(
        self,
        db: &'db dyn Db,
    ) -> Option<ValueType<'db>> {
        let hir = self.hir(db);
        let type_ = hir.ret_type(db);
        if type_.is_unit() {
            return None;
        }
        match lower_type(db, &type_, hir.module(db).krate(db))
            .expect("functions with unsupported return types are not lowered")
        {
            Type::Value(type_) => Some(type_),
            Type::Borrowed { .. } => {
                unreachable!("functions returning borrowed values are not lowered")
            }
        }
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_docs(self, db: &'db dyn Db) -> Option<String> {
        self.hir(db).hir_docs(db).map(|docs| docs.docs().to_owned())
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_span(self, db: &'db dyn Db) -> Option<Span> {
        lower_item_span(db, self.hir(db), self.hir(db).module(db).krate(db))
    }
}

pub(super) fn lower_function<'db>(
    db: &'db dyn Db,
    hir: ra_ap_hir::Function,
) -> Result<Function<'db>, Box<FunctionLowerError>> {
    let crate_ = hir.module(db).krate(db);
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
    if !GenericDef::Function(hir).params(db).is_empty() {
        causes.push(FunctionLowerCause::UnsupportedGeneric);
    }
    if let Some(source) = hir.source(db) {
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

    for (index, param) in hir.params_without_self(db).into_iter().enumerate() {
        let name = param
            .name(db)
            .map(|name| name.display(db, crate_.edition(db)).to_string())
            .unwrap_or_else(|| format!("arg{index}"));
        match lower_type(db, param.ty(), crate_) {
            Err(error) => causes.push(FunctionLowerCause::UnsupportedParam {
                name,
                causes: vec![error],
            }),
            Ok(Type::Value(ValueType::Struct(struct_))) if !is_clonable(db, param.ty(), crate_) => {
                causes.push(FunctionLowerCause::NonClonableMovedParam {
                    name,
                    type_fqn: struct_.fqn(db).to_owned(),
                });
            }
            Ok(Type::Value(ValueType::Enum(enum_))) if !is_clonable(db, param.ty(), crate_) => {
                causes.push(FunctionLowerCause::NonClonableMovedParam {
                    name,
                    type_fqn: enum_.fqn(db).to_owned(),
                });
            }
            Ok(Type::Borrowed {
                is_mutable: true,
                inner:
                    ValueType::bool
                    | ValueType::char
                    | ValueType::i8
                    | ValueType::u8
                    | ValueType::i16
                    | ValueType::u16
                    | ValueType::i32
                    | ValueType::u32
                    | ValueType::f32
                    | ValueType::f64
                    | ValueType::Str
                    | ValueType::String,
            }) => {
                let span = lower_item_span(db, hir, crate_);
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

    let return_type = hir.ret_type(db);
    if !return_type.is_unit() {
        match lower_type(db, &return_type, crate_) {
            Ok(Type::Borrowed { .. }) => causes.push(FunctionLowerCause::BorrowedReturn),
            Err(error) => causes.push(FunctionLowerCause::Type(error)),
            Ok(Type::Value(_)) => {}
        }
    }

    if causes.is_empty() {
        Ok(Function::new(db, hir))
    } else {
        let span = lower_item_span(db, hir, crate_);
        let (source_code, span) = diagnostic_location(db, span.as_ref());
        Err(Box::new(FunctionLowerError {
            fqn: function_fqn(db, hir),
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
    span: Option<DiagSpan>,
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
    span: Option<DiagSpan>,
}
