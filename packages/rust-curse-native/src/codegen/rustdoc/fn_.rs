use miette::Diagnostic;
use rustdoc_types::{Abi, ItemEnum, ItemKind as RItemKind};
use thiserror::Error;

use crate::codegen::{
    db::Db,
    diag::DiagAccumulateExt as _,
    ir::{
        fn_::{Fn, FnId, FnParam, ParamMode},
        item::{ItemId, ItemIdExt as _, ItemKind, ItemPath},
        span::{SourceCode, Span},
        type_::Type,
    },
    rustdoc::{
        CrateExt, ItemIdExt as _, RustdocId,
        enum_::is_enum_clonable,
        shape::is_struct_clonable,
        type_::{LoweredType, TypeLowerError, lower_type},
    },
};

/// Lowers the rustdoc function.
#[salsa::tracked]
pub fn lower_fn<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> Result<Fn<'db>, Box<FnLowerError>> {
    let item = id.item(db);
    let crate_body = id.crate_body(db);

    let def = match &item.inner {
        ItemEnum::Function(def) => def,
        _ => unreachable!(),
    };

    assert!(def.has_body, "trait/extern functions cannot be lowerd");

    let item_summary = crate_body.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, RItemKind::Function);

    let mut causes = Vec::new();

    if def.header.is_async {
        causes.push(FnLowerError::UnsupportedAsync);
    }

    if def.header.is_unsafe {
        causes.push(FnLowerError::UnsupportedUnsafe);
    }

    if def.header.abi != Abi::Rust {
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

    let mut params = Vec::new();

    for (name, ty) in &def.sig.inputs {
        let lowered = match lower_type(db, crate_body.ty(db, ty)) {
            Ok(lowered) => lowered,
            Err(diags) => {
                causes.push(FnLowerError::UnsupportedParam {
                    name: name.clone(),
                    causes: diags,
                });
                continue;
            }
        };

        let (type_, mode) = match lowered {
            LoweredType::Moved(type_) => (type_, ParamMode::Move),
            LoweredType::Borrowed { type_, is_mutable } => (
                type_,
                if is_mutable {
                    ParamMode::Mut
                } else {
                    ParamMode::Ref
                },
            ),
        };

        if mode == ParamMode::Move {
            match type_ {
                Type::Struct(struct_id) if !is_struct_clonable(db, struct_id.rustdoc_id(db)) => {
                    causes.push(FnLowerError::NonClonableMovedStructParam {
                        name: name.clone(),
                        struct_fqn: struct_id.fqn(db).to_owned(),
                    });
                    continue;
                }
                Type::Enum(enum_id) if !is_enum_clonable(db, enum_id.rustdoc_id(db)) => {
                    causes.push(FnLowerError::NonClonableMovedEnumParam {
                        name: name.clone(),
                        enum_fqn: enum_id.fqn(db).to_owned(),
                    });
                    continue;
                }
                Type::Primitive(_) | Type::Str | Type::String | Type::Struct(_) | Type::Enum(_) => {
                }
            }
        }

        if mode == ParamMode::Mut {
            match type_ {
                Type::Primitive(_) => {
                    let crate_ = id.crate_(db);
                    FnLowerError::PrimitiveMutRefIgnored {
                        name: name.clone(),
                        source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
                        span: crate_.read_span_opt(item.span.as_ref()),
                    }
                    .accumulate(db);
                }
                Type::Str | Type::String => {
                    let crate_ = id.crate_(db);
                    FnLowerError::StringMutRefIgnored {
                        name: name.clone(),
                        source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
                        span: crate_.read_span_opt(item.span.as_ref()),
                    }
                    .accumulate(db);
                }
                Type::Struct(_) | Type::Enum(_) => {}
            }
        }

        params.push(FnParam::new(db, name.clone(), type_, mode));
    }

    let ret_type = match &def.sig.output {
        None => None,
        Some(ty) => match lower_type(db, crate_body.ty(db, ty)) {
            Ok(LoweredType::Moved(type_)) => Some(type_),
            Ok(LoweredType::Borrowed { .. }) => {
                causes.push(FnLowerError::BorrowedReturn);
                None
            }
            Err(diags) => {
                causes.extend(diags.into_iter().map(FnLowerError::Type));
                None
            }
        },
    };

    if !causes.is_empty() {
        let crate_ = id.crate_(db);
        return Err(Box::new(FnLowerError::Skipped {
            fn_fqn: item_summary.path.join("::"),
            source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
            span: crate_.read_span_opt(item.span.as_ref()),
            causes,
        }));
    }

    let id = FnId::new(
        db,
        ItemId::new(db, ItemPath::new(db, &item_summary.path), ItemKind::Fn),
    )
    .expect("newly created function item must have function kind");

    Ok(Fn::new(db, id, params, ret_type, item.docs.clone()))
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum FnLowerError {
    #[error("the function \"{fn_fqn}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_fn), severity(Warning))]
    Skipped {
        /// The fully qualified name of the function being lowered.
        fn_fqn: String,
        #[source_code]
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
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
    UnsupportedAbi { abi: Abi },

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
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
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
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
    },
}
