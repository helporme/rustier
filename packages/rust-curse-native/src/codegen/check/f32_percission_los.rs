use miette::Diagnostic;
use thiserror::Error;

use crate::codegen::{
    db::Db,
    diag::{DiagSource, DiagSpan},
    ir::{
        enum_::Enum,
        fn_::Fn,
        shape::Shape,
        struct_::Struct,
        type_::{IrPrimitive, Type},
    },
    rustdoc::{
        ItemIdExt as _,
        mod_::{all_mod_ids, lower_mod_enums, lower_mod_fns, lower_mod_structs},
        span::RustdocSpanExt as _,
    },
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

        for enum_ in lower_mod_enums(db, *mod_id) {
            if let Some(diag) = check_f32_percission_los_enum(db, *enum_) {
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
        .any(|param| param.type_(db) == Type::Primitive(IrPrimitive::f32));
    let has_f32_return = fn_.return_type(db) == Some(Type::Primitive(IrPrimitive::f32));

    if has_f32_param || has_f32_return {
        let id = fn_.id(db).rustdoc_id(db);
        let item = id.item(db);
        let crate_ = id.crate_(db);
        let source_code = item
            .span
            .as_ref()
            .map(|span| span.diag_source(db, crate_))
            .into();
        let span = item.span.as_ref().map(|span| span.diag_span(db, crate_));

        return Some(F32PrecisionLoss { source_code, span });
    }

    None
}

fn check_f32_percission_los_struct(db: &dyn Db, struct_: Struct<'_>) -> Option<F32PrecisionLoss> {
    let has_f32_field = match struct_.shape(db) {
        Shape::Unit => false,
        Shape::Tuple(fields) => fields
            .iter()
            .flatten()
            .any(|field| field.type_(db) == Type::Primitive(IrPrimitive::f32)),
        Shape::Plain { fields, .. } => fields
            .iter()
            .any(|field| field.type_(db) == Type::Primitive(IrPrimitive::f32)),
    };

    if has_f32_field {
        let id = struct_.id(db).rustdoc_id(db);
        let item = id.item(db);
        let crate_ = id.crate_(db);
        let source_code = item
            .span
            .as_ref()
            .map(|span| span.diag_source(db, crate_))
            .into();
        let span = item.span.as_ref().map(|span| span.diag_span(db, crate_));

        return Some(F32PrecisionLoss { source_code, span });
    }

    None
}

fn check_f32_percission_los_enum(db: &dyn Db, enum_: Enum<'_>) -> Option<F32PrecisionLoss> {
    let has_f32_field = enum_.variants(db).iter().any(|variant| {
        variant
            .fields(db)
            .any(|field| field.type_(db) == Type::Primitive(IrPrimitive::f32))
    });

    if has_f32_field {
        let id = enum_.id(db).rustdoc_id(db);
        let item = id.item(db);
        let crate_ = id.crate_(db);
        let source_code = item
            .span
            .as_ref()
            .map(|span| span.diag_source(db, crate_))
            .into();
        let span = item.span.as_ref().map(|span| span.diag_span(db, crate_));

        return Some(F32PrecisionLoss { source_code, span });
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
    source_code: DiagSource,
    #[label]
    span: Option<DiagSpan>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::test::test_db;

    fn assert_f32_precision_loss(db: &dyn Db) {
        let diags = check_f32_percission_los(db);

        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code().map(|code| code.to_string()).as_deref(),
            Some("rust_curse::f32_precision_loss")
        );
    }

    #[test]
    fn fn_returns_f32() {
        let db = test_db! {
            pub fn measurement() -> f32 {
                1.25
            }
        };

        assert_f32_precision_loss(&db);
    }

    #[test]
    fn fn_accepts_f32() {
        let db = test_db! {
            pub fn measurement(value: f32) {

            }
        };

        assert_f32_precision_loss(&db);
    }

    #[test]
    fn struct_with_f32() {
        let db = test_db! {
            pub struct Measurement {
                pub value: f32,
            }
        };

        assert_f32_precision_loss(&db);
    }

    #[test]
    fn enum_with_tuple_variant_f32() {
        let db = test_db! {
            pub enum Measurement {
                Value(f32),
            }
        };

        assert_f32_precision_loss(&db);
    }

    #[test]
    fn enum_with_struct_variant_f32() {
        let db = test_db! {
            pub enum Measurement {
                Value { value: f32 },
            }
        };

        assert_f32_precision_loss(&db);
    }
}
