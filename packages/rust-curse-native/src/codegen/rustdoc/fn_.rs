use rustdoc_types::{Abi, ItemEnum, ItemKind};
use salsa::Accumulator;

use crate::{
    codegen::{
        db::Db,
        ir,
        rustdoc::{lower_type, CrateExt, Id, LoweredType},
    },
    diag::{Diag, Severity},
};

/// Lowers the rustdoc function.
#[salsa::tracked]
pub fn lower_fn<'db>(db: &'db dyn Db, id: Id<'db>) -> Result<ir::Fn<'db>, Vec<Diag>> {
    let item = id.item(db);
    let crate_ = id.crate_(db);

    let def = match &item.inner {
        ItemEnum::Function(def) => def,
        _ => unreachable!(),
    };

    assert!(def.has_body, "trait/extern functions cannot be lowerd");

    let span: Option<ir::Span> = item.span.clone().map(Into::into);
    let mut causes = Vec::new();

    if def.header.is_async {
        causes.push(Diag::unsupported_async_fn());
    }

    if def.header.is_unsafe {
        causes.push(Diag::unsupported_unsafe_fn());
    }

    if def.header.abi != Abi::Rust {
        causes.push(Diag::unsupported_abi(&def.header.abi));
    }

    if def.sig.is_c_variadic {
        causes.push(Diag::unsupported_variadic_fn());
    }

    if !def.generics.params.is_empty() {
        causes.push(Diag::unsupported_generic_fn());
    }

    let mut params = Vec::new();

    for (name, ty) in &def.sig.inputs {
        let lowered = match lower_type(db, crate_.ty(db, ty)) {
            Ok(lowered) => lowered,
            Err(diags) => {
                causes.push(Diag::unsupported_param(name).with_causes(diags));
                continue;
            }
        };

        let (type_, mode) = match lowered {
            LoweredType::Moved(type_) => (type_, ir::ParamMode::Move),
            LoweredType::Borrowed { type_, is_mutable } => (
                type_,
                if is_mutable {
                    ir::ParamMode::Mut
                } else {
                    ir::ParamMode::Ref
                },
            ),
        };

        if matches!(type_, ir::Type::Primitive(_)) && mode == ir::ParamMode::Mut {
            Diag::primitive_mut_ref_ignored(name)
                .try_add_span(span.clone())
                .accumulate(db);
        }

        params.push(ir::FnParam::new(db, name.clone(), type_, mode));
    }

    let ret_type = match &def.sig.output {
        None => None,
        Some(ty) => match lower_type(db, crate_.ty(db, ty)) {
            Ok(LoweredType::Moved(type_)) => Some(type_),
            Ok(LoweredType::Borrowed { .. }) => {
                causes.push(Diag::borrowed_return());
                None
            }
            Err(diags) => {
                causes.extend(diags);
                None
            }
        },
    };

    if !causes.is_empty() {
        let name = item.name.as_deref().unwrap_or_default();
        let diag = Diag::skipped_fn(name)
            .with_causes(causes)
            .try_add_span(span);

        return Err(vec![diag]);
    }

    let item_summary = crate_.paths.get(&item.id).unwrap();
    assert_eq!(item_summary.kind, ItemKind::Function);
    let id = ir::Id::new(db, &item_summary.path);

    Ok(ir::Fn::new(
        db,
        id,
        params,
        ret_type,
        item.docs.clone(),
        span,
    ))
}

impl Diag {
    pub fn unsupported_async_fn() -> Diag {
        Diag::new(
            "rust-curse::unsupported_async_fn",
            "async functions are not supported",
            Severity::Warning,
        )
    }

    pub fn unsupported_unsafe_fn() -> Diag {
        Diag::new(
            "rust-curse::unsupported_unsafe_fn",
            "unsafe functions are not supported",
            Severity::Warning,
        )
        .with_help("wrap the unsafe operations in a safe function and export that instead")
    }

    pub fn unsupported_abi(abi: &Abi) -> Diag {
        Diag::new(
            "rust-curse::unsupported_abi",
            format!("functions with the {abi:?} ABI are not supported"),
            Severity::Warning,
        )
    }

    pub fn unsupported_variadic_fn() -> Diag {
        Diag::new(
            "rust-curse::unsupported_variadic_fn",
            "C-variadic functions are not supported",
            Severity::Warning,
        )
    }

    pub fn unsupported_generic_fn() -> Diag {
        Diag::new(
            "rust-curse::unsupported_generic_fn",
            "generic functions are not supported",
            Severity::Warning,
        )
        .with_help(
            "export a non-generic wrapper that instantiates the generic parameters with concrete types",
        )
    }

    pub fn unsupported_param(name: &str) -> Diag {
        Diag::new(
            "rust-curse::unsupported_param",
            format!("the parameter `{name}` has an unsupported type"),
            Severity::Warning,
        )
    }

    pub fn borrowed_return() -> Diag {
        Diag::new(
            "rust-curse::borrowed_return",
            "functions cannot return borrowed values",
            Severity::Warning,
        )
        // todo: enrich help with Box<> etc. once they are supported
        .with_help("return an owned value instead, for example via `.to_owned()` or `.clone()`")
    }

    pub fn skipped_fn(name: &str) -> Diag {
        Diag::new(
            "rust-curse::skipped_fn",
            format!("the function `{name}` cannot be exported to JS"),
            Severity::Warning,
        )
    }

    pub fn primitive_mut_ref_ignored(name: &str) -> Diag {
        Diag::new(
            "rust-curse::primitive_mut_ref_ignored",
            format!("the mutable reference to the primitive parameter `{name}` does not propagate changes"),
            Severity::Warning,
        )
        .with_help("primitive values are copied across the JS boundary, so mutations made in Rust are not reflected in JS")
    }
}
