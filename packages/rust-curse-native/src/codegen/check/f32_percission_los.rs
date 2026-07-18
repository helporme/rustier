use miette::Diagnostic;
use thiserror::Error;

use crate::codegen::{
    db::Db,
    ir::{
        fn_::Fn,
        shape::Shape,
        span::{SourceCode, Span},
        struct_::Struct,
        type_::{Primitive, Type},
    },
    rustdoc::mod_::{all_mod_ids, lower_mod_fns, lower_mod_structs},
};

/// Emits warnings for exported items that pass f32 values across the JS
/// boundary.
#[salsa::tracked(returns(ref))]
pub fn check_f32_percission_los(db: &dyn Db) -> Vec<F32PrecisionLoss> {
    let mut diags = Vec::new();

    for mod_id in all_mod_ids(db) {
        for fn_ in lower_mod_fns(db, *mod_id) {
            if let Some(diag) = check_f32_percission_los_fn(db, *fn_) {
                diags.push(diag);
            }
        }

        for struct_ in lower_mod_structs(db, *mod_id) {
            if let Some(diag) = check_f32_percission_los_struct(db, *struct_) {
                diags.push(diag);
            }
        }
    }

    diags
}

fn check_f32_percission_los_fn(db: &dyn Db, fn_: Fn<'_>) -> Option<F32PrecisionLoss> {
    let has_f32_param = fn_
        .params(db)
        .iter()
        .any(|param| param.type_(db) == Type::Primitive(Primitive::f32));
    let has_f32_return = fn_.return_type(db) == Some(Type::Primitive(Primitive::f32));

    if has_f32_param || has_f32_return {
        let id = fn_.id(db).rustdoc_id(db);
        let item = id.item(db);
        let crate_ = id.crate_(db);

        return Some(F32PrecisionLoss {
            source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
            span: crate_.read_span_opt(item.span.as_ref()),
        });
    }

    None
}

fn check_f32_percission_los_struct(db: &dyn Db, struct_: Struct<'_>) -> Option<F32PrecisionLoss> {
    let has_f32_field = match struct_.shape(db) {
        Shape::Unit => false,
        Shape::Tuple(fields) => fields
            .iter()
            .flatten()
            .any(|field| field.type_(db) == Type::Primitive(Primitive::f32)),
        Shape::Plain { fields, .. } => fields
            .iter()
            .any(|field| field.type_(db) == Type::Primitive(Primitive::f32)),
    };

    if has_f32_field {
        let id = struct_.id(db).rustdoc_id(db);
        let item = id.item(db);
        let crate_ = id.crate_(db);

        return Some(F32PrecisionLoss {
            source_code: crate_.read_source_code_for_span_opt(item.span.as_ref()),
            span: crate_.read_span_opt(item.span.as_ref()),
        });
    }

    None
}

/// Warning emitted when f32 values cross the Rust-to-JavaScript boundary.
#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("passing f32 values between Rust and JS may lose precision")]
#[diagnostic(
    code(rust_curse::f32_precision_loss),
    help("use f64 instead to avoid precision loss across the JS boundary"),
    severity(Warning)
)]
pub struct F32PrecisionLoss {
    #[source_code]
    source_code: SourceCode,
    #[label]
    span: Option<Span>,
}
