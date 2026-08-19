use miette::Diagnostic;
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource, DiagSpan},
    ir::rust::{Enum, Function, Span, Struct, ValueType},
};

#[salsa::tracked]
impl<'db> Function<'db> {
    #[salsa::tracked]
    pub(super) fn check_f32_precision_loss(self, db: &'db dyn Db) {
        let has_f32_param = self
            .params(db)
            .iter()
            .any(|param| param.type_(db).deborrow() == ValueType::f32);
        let has_f32_return = self.ret_type(db) == Some(ValueType::f32);

        if has_f32_param || has_f32_return {
            emit(db, self.span(db));
        }
    }
}

#[salsa::tracked]
impl<'db> Struct<'db> {
    #[salsa::tracked]
    pub(super) fn check_f32_precision_loss(self, db: &'db dyn Db) {
        let has_f32_field = self
            .fields(db)
            .iter()
            .any(|field| field.value_type(db) == ValueType::f32);

        if has_f32_field {
            emit(db, self.span(db));
        }
    }
}

#[salsa::tracked]
impl<'db> Enum<'db> {
    #[salsa::tracked]
    pub(super) fn check_f32_precision_loss(self, db: &'db dyn Db) {
        let has_f32_field = self
            .fields(db)
            .iter()
            .any(|field| field.value_type(db) == ValueType::f32);

        if has_f32_field {
            emit(db, self.span(db));
        }
    }
}

fn emit(db: &dyn Db, span: &Span) {
    F32PrecisionLoss {
        source_code: span.diag_source(db),
        span: span.diag_span(db),
    }
    .accumulate(db);
}

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
    span: DiagSpan,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compiler::check::tests::diagnostic_codes, test_db};

    fn assert_f32_precision_loss(db: &dyn Db) {
        assert_eq!(diagnostic_codes(db), ["rust_curse::f32_precision_loss"]);
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
    fn fn_accepts_borrowed_f32() {
        let db = test_db! {
            pub fn measurement(value: &f32) {

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
