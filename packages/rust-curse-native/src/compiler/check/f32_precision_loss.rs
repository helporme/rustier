use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::rust::{Callable, Enum, Span, Struct, TypeKind},
};

#[salsa::tracked]
impl<'db> Callable<'db> {
    #[salsa::tracked]
    pub(super) fn check_f32_precision_loss(self, db: &'db dyn Db) {
        let f32_param = self
            .params(db)
            .iter()
            .find(|param| param.type_(db).deborrow(db).kind(db) == TypeKind::f32);
        let has_f32_return = self.ret_type(db).map(|type_| type_.kind(db)) == Some(TypeKind::f32);

        if let Some(param) = f32_param {
            emit(db, param.span(db).as_ref());
        } else if has_f32_return {
            emit(db, self.declaration(db).span(db));
        }
    }
}

#[salsa::tracked]
impl<'db> Struct<'db> {
    #[salsa::tracked]
    pub(super) fn check_f32_precision_loss(self, db: &'db dyn Db) {
        let f32_field = self
            .fields(db)
            .find(|field| field._type(db).kind(db) == TypeKind::f32);

        if let Some(field) = f32_field {
            emit(db, field.span(db));
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
            .any(|field| field._type(db).kind(db) == TypeKind::f32);

        if has_f32_field {
            emit(db, self.span(db));
        }
    }
}

fn emit(db: &dyn Db, span: Option<&Span>) {
    F32PrecisionLoss {
        source_code: span
            .map(|span| span.src(db))
            .map(Into::into)
            .unwrap_or_default(),
        span: span.map(|span| (span.offset, span.length).into()),
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
    span: Option<SourceSpan>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compiler::check::tests::diagnostic_codes, diagnostic_test_db as test_db};

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
            #[derive(Clone)]
            pub struct Measurement {
                pub value: f32,
            }
        };

        assert_f32_precision_loss(&db);
    }

    #[test]
    fn enum_with_tuple_variant_f32() {
        let db = test_db! {
            #[derive(Clone)]
            pub enum Measurement {
                Value(f32),
            }
        };

        assert_f32_precision_loss(&db);
    }

    #[test]
    fn enum_with_struct_variant_f32() {
        let db = test_db! {
            #[derive(Clone)]
            pub enum Measurement {
                Value { value: f32 },
            }
        };

        assert_f32_precision_loss(&db);
    }
}
