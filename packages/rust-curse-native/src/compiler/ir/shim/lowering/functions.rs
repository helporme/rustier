use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::{
        rust::{AsItem as _, Callable, FunctionQualified, Type, TypeKind},
        shim::{ShimFunction, ShimFunctionAssociated, ShimFunctionFree},
    },
};

#[salsa::tracked]
impl<'db> Callable<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::shim) fn lower(
        self,
        db: &'db dyn Db,
    ) -> Result<ShimFunction<'db>, Box<FunctionLowerError>> {
        let mut causes = Vec::new();
        let parameters = self
            .receiver_type(db)
            .into_iter()
            .map(|type_| ("self", type_))
            .chain(
                self.params(db)
                    .iter()
                    .map(|param| (param.name(db).as_str(), *param.type_(db))),
            );

        for (name, type_) in parameters {
            if type_.deborrow(db).contains_borrowed_values(db) {
                causes.push(FunctionLowerCause::BorrowedParam {
                    name: name.to_owned(),
                    type_fqn: type_.display(db).to_owned(),
                });
                continue;
            }
            if matches!(type_.kind(db), TypeKind::Struct(_) | TypeKind::Enum(_))
                && type_.is_clonable(db) == Some(false)
            {
                causes.push(FunctionLowerCause::NonClonableMovedParam {
                    name: name.to_owned(),
                    type_fqn: type_.display(db).to_owned(),
                });
            }
        }

        if self
            .ret_type(db)
            .is_some_and(|type_| type_.contains_borrowed_values(db))
        {
            causes.push(FunctionLowerCause::BorrowedReturn);
        }

        if !causes.is_empty() {
            let source_span = self.declaration(db).span(db);
            return Err(Box::new(FunctionLowerError {
                fqn: self.fqn(db).to_owned(),
                source_code: source_span
                    .map(|span| span.src(db).into())
                    .unwrap_or_default(),
                span: source_span.map(|span| (span.offset, span.length).into()),
                causes,
            }));
        }

        Ok(match self.function(db) {
            FunctionQualified::Free(_) => ShimFunctionFree::new(db, self).into(),
            FunctionQualified::Inherent(_) => ShimFunctionAssociated::new(db, self).into(),
        })
    }
}

#[salsa::tracked]
impl<'db> Type<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler) fn lower_methods(
        self,
        db: &'db dyn Db,
    ) -> Vec<ShimFunctionAssociated<'db>> {
        self.impls(db)
            .iter()
            .flat_map(|impl_| impl_.functions(db))
            .filter(|function| {
                let declaration = function.declaration(db);
                declaration.receiver(db).is_some() || declaration.name(db) != "prototype"
            })
            .filter_map(|function| {
                let callable = function.as_callible(db)?;
                match callable.lower(db) {
                    Ok(ShimFunction::Associated(function)) => Some(function),
                    Ok(_) => unreachable!("inherent functions lower to associated shim functions"),
                    Err(error) => {
                        (*error).accumulate(db);
                        None
                    }
                }
            })
            .collect()
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("the function \"{fqn}\" cannot be exported to JS")]
#[diagnostic(code(rust_curse::skipped_fn), severity(Warning))]
pub(in crate::compiler::ir::shim) struct FunctionLowerError {
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
    #[error("the parameter `{name}` has type \"{type_fqn}\", which contains borrowed values")]
    #[diagnostic(
        code(rust_curse::borrowed_param),
        help("use owned values inside the ADT; a reference to an owned ADT is supported"),
        severity(Warning)
    )]
    BorrowedParam { name: String, type_fqn: String },

    #[error("the parameter `{name}` moves \"{type_fqn}\", which does not implement `Clone`")]
    #[diagnostic(
        code(rust_curse::non_clonable_moved_param),
        help("pass the value by reference or implement `Clone` for the type"),
        severity(Warning)
    )]
    NonClonableMovedParam { name: String, type_fqn: String },

    #[error("functions cannot return borrowed values")]
    #[diagnostic(
        code(rust_curse::borrowed_return),
        help(
            "return owned values, including inside ADTs; cloning an ADT containing references does not remove its borrows"
        ),
        severity(Warning)
    )]
    BorrowedReturn,
}
