use miette::Diagnostic;
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource, DiagSpan},
    ir::rust::{AsItem as _, Function, Mod, Param, Type, ValueType},
};

use super::{
    ItemExt as _, RustdocId, RustdocSpan, RustdocType, fields::is_clonable, types::TypeLowerError,
};

use rustdoc_types as doc;

#[salsa::tracked]
impl<'db> Function<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir) fn lower(
        db: &'db dyn Db,
        id: RustdocId<'db>,
        parent: Mod<'db>,
    ) -> Result<Self<'db>, Box<FnLowerError>> {
        let item = id.item(db);

        let doc::ItemEnum::Function(def) = &item.inner else {
            unreachable!("expected function, got {:?}", item.inner);
        };

        let mut causes = Vec::new();

        if def.header.is_async {
            causes.push(FnLowerError::UnsupportedAsync);
        }

        if def.header.is_unsafe {
            causes.push(FnLowerError::UnsupportedUnsafe);
        }

        if def.header.abi != doc::Abi::Rust {
            causes.push(FnLowerError::UnsupportedAbi {
                abi: def.header.abi.clone(),
            });
        }

        if def.sig.is_c_variadic {
            causes.push(FnLowerError::UnsupportedVariadic);
        }

        if !def.generics.params.is_empty() {
            causes.push(FnLowerError::UnsupportedGeneric);
        }

        for (name, ty) in &def.sig.inputs {
            let type_ = match Type::lower(db, RustdocType::new(db, id.crate_name(db), ty), parent) {
                Ok(lowered) => lowered,
                Err(diags) => {
                    causes.push(FnLowerError::UnsupportedParam {
                        name: name.clone(),
                        causes: diags,
                    });
                    continue;
                }
            };

            match type_ {
                Type::Value(type_) => match type_ {
                    ValueType::Struct(struct_id) if !is_clonable(db, struct_id.rustdoc_id(db)) => {
                        causes.push(FnLowerError::NonClonableMovedStructParam {
                            name: name.clone(),
                            struct_fqn: struct_id.fqn(db).to_owned(),
                        });
                        continue;
                    }
                    ValueType::Enum(enum_id) if !is_clonable(db, enum_id.rustdoc_id(db)) => {
                        causes.push(FnLowerError::NonClonableMovedEnumParam {
                            name: name.clone(),
                            enum_fqn: enum_id.fqn(db).to_owned(),
                        });
                        continue;
                    }
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
                    | ValueType::String
                    | ValueType::Struct(_)
                    | ValueType::Enum(_) => {}
                },
                Type::Borrowed {
                    is_mutable: true,
                    inner,
                } => match inner {
                    ValueType::bool
                    | ValueType::char
                    | ValueType::i8
                    | ValueType::u8
                    | ValueType::i16
                    | ValueType::u16
                    | ValueType::i32
                    | ValueType::u32
                    | ValueType::f32
                    | ValueType::f64 => {
                        let span = item.span.as_ref().map(|span| {
                            let crate_name = id.crate_name(db);
                            RustdocSpan::new(db, span, crate_name).lower(db)
                        });
                        let source_code = span.as_ref().map(|span| span.diag_source(db)).into();
                        let span = span.as_ref().map(|span| span.diag_span(db));
                        FnLowerError::PrimitiveMutRefIgnored {
                            name: name.clone(),
                            source_code,
                            span,
                        }
                        .accumulate(db);
                    }
                    ValueType::Str | ValueType::String => {
                        let span = item.span.as_ref().map(|span| {
                            let crate_name = id.crate_name(db);
                            RustdocSpan::new(db, span, crate_name).lower(db)
                        });
                        let source_code = span.as_ref().map(|span| span.diag_source(db)).into();
                        let span = span.as_ref().map(|span| span.diag_span(db));
                        FnLowerError::StringMutRefIgnored {
                            name: name.clone(),
                            source_code,
                            span,
                        }
                        .accumulate(db);
                    }
                    ValueType::Struct(_) | ValueType::Enum(_) => {}
                },
                _ => {}
            };
        }

        match &def.sig.output {
            Some(ty) => {
                match Type::lower(db, RustdocType::new(db, id.crate_name(db), ty), parent) {
                    Ok(Type::Borrowed { .. }) => {
                        causes.push(FnLowerError::BorrowedReturn);
                    }
                    Err(diags) => {
                        causes.extend(diags.into_iter().map(FnLowerError::Type));
                    }
                    Ok(Type::Value(_)) => {}
                }
            }
            None => {}
        };

        if !causes.is_empty() {
            let span = item.span.as_ref().map(|span| {
                let crate_name = id.crate_name(db);
                RustdocSpan::new(db, span, crate_name).lower(db)
            });
            let source_code = span.as_ref().map(|span| span.diag_source(db)).into();
            let span = span.as_ref().map(|span| span.diag_span(db));

            return Err(Box::new(FnLowerError::Skipped {
                fn_fqn: id.summary(db).path.join("::"),
                source_code,
                span,
                causes,
            }));
        }

        let name = item.name.as_ref().unwrap();
        Ok(Self::new(db, parent, name))
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir) fn lower_ret_type(self, db: &'db dyn Db) -> Option<ValueType<'db>> {
        let id = self.rustdoc_id(db);
        let item = id.item(db);

        let doc::ItemEnum::Function(def) = &item.inner else {
            unreachable!("expected function, got {:?}", item.inner);
        };

        def.sig.output.as_ref().map(|ty| {
            Type::lower(
                db,
                RustdocType::new(db, id.crate_name(db), ty),
                self.parent(db),
            )
            .expect("ret type must be lowerable otherwise fn won't be lowered")
            .deborrow()
        })
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir) fn lower_params(self, db: &'db dyn Db) -> Vec<Param<'db>> {
        let id = self.rustdoc_id(db);
        let item = id.item(db);

        let doc::ItemEnum::Function(def) = &item.inner else {
            unreachable!("expected function, got {:?}", item.inner);
        };

        def.sig
            .inputs
            .iter()
            .enumerate()
            .map(|(index, (name, ty))| {
                let ty = Type::lower(
                    db,
                    RustdocType::new(db, id.crate_name(db), ty),
                    self.parent(db),
                )
                .expect("ret type must be lowerable otherwise fn won't be lowered");

                Param::new(db, self, index, name.clone(), ty)
            })
            .collect()
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum FnLowerError {
    #[error("the function \"{fn_fqn}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_fn), severity(Warning))]
    Skipped {
        /// The fully qualified name of the function being lowered.
        fn_fqn: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
        #[related]
        causes: Vec<FnLowerError>,
    },

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

    #[error("functions with the {abi:?} ABI are not supported")]
    #[diagnostic(code(rust_curse::unsupported_abi), severity(Warning))]
    UnsupportedAbi { abi: doc::Abi },

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

    #[error("the parameter `{name}` has an unsupported type")]
    #[diagnostic(code(rust_curse::unsupported_param), severity(Warning))]
    UnsupportedParam {
        /// The name of the parameter that has an unsupported type.
        name: String,

        #[related]
        causes: Vec<TypeLowerError>,
    },

    #[error(transparent)]
    #[diagnostic(transparent)]
    Type(#[from] TypeLowerError),

    #[error("functions cannot return borrowed values")]
    #[diagnostic(
        code(rust_curse::borrowed_return),
        help("return an owned value instead, for example via `.to_owned()` or `.clone()`"),
        severity(Warning)
    )]
    BorrowedReturn,

    #[error(
        "the moved struct parameter `{name}` cannot be exported because \"{struct_fqn}\" does not implement `Clone`"
    )]
    #[diagnostic(
        code(rust_curse::non_clonable_moved_struct_param),
        help(
            "derive or implement `Clone` for \"{struct_fqn}\", or pass the parameter by reference"
        ),
        severity(Warning)
    )]
    NonClonableMovedStructParam {
        /// The name of the non-clonable moved struct parameter.
        name: String,
        /// The fully qualified name of the parameter's struct.
        struct_fqn: String,
    },

    #[error(
        "the moved enum parameter `{name}` cannot be exported because \"{enum_fqn}\" does not implement `Clone`"
    )]
    #[diagnostic(
        code(rust_curse::non_clonable_moved_enum_param),
        help("derive or implement `Clone` for \"{enum_fqn}\", or pass the parameter by reference"),
        severity(Warning)
    )]
    NonClonableMovedEnumParam {
        /// The name of the non-clonable moved enum parameter.
        name: String,
        /// The fully qualified name of the parameter's enum.
        enum_fqn: String,
    },

    #[error("the mutable reference to the primitive parameter `{name}` does not propagate changes")]
    #[diagnostic(
        code(rust_curse::primitive_mut_ref_ignored),
        help(
            "primitive values are copied across the JS boundary, so mutations made in Rust are not reflected in JS"
        ),
        severity(Warning)
    )]
    PrimitiveMutRefIgnored {
        /// The name of the parameter that has an ignored mutable reference.
        name: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
    },

    #[error("the mutable reference to the string parameter `{name}` does not propagate changes")]
    #[diagnostic(
        code(rust_curse::string_mut_ref_ignored),
        help(
            "string values are copied across the JS boundary, so mutations made in Rust are not reflected in JS"
        ),
        severity(Warning)
    )]
    StringMutRefIgnored {
        /// The name of the parameter that has an ignored mutable reference.
        name: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
    },
}
