use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::{
        rust::{AsItem as _, Enum, Impl, Struct},
        shim::ShimFunctionAssociated,
    },
};

#[salsa::tracked]
impl<'db> Struct<'db> {
    #[salsa::tracked]
    pub(super) fn check_method_field_collisions(self, db: &'db dyn Db) {
        check_method_field_collisions(db, self.impls(db));
    }
}

#[salsa::tracked]
impl<'db> Enum<'db> {
    #[salsa::tracked]
    pub(super) fn check_method_field_collisions(self, db: &'db dyn Db) {
        check_method_field_collisions(db, self.impls(db));
    }
}

fn check_method_field_collisions<'db>(db: &'db dyn Db, impls: &[Impl<'db>]) {
    for function in impls.iter().flat_map(|impl_| impl_.functions(db)) {
        let method = ShimFunctionAssociated::new(db, *function);
        if !method.is_shadowed_by_field(db) {
            continue;
        }

        let source_span = function.span(db);
        let source_code = source_span
            .map(|span| span.src(db))
            .map(Into::into)
            .unwrap_or_default();
        let span = source_span.map(|span| (span.offset, span.length).into());

        MethodShadowedByField {
            fqn: function.fqn(db).to_owned(),
            source_code,
            span,
        }
        .accumulate(db);
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error(
    "the instance method \"{fqn}\" is not exported to JS because it is shadowed by a field with the same name"
)]
#[diagnostic(
    code(rust_curse::method_shadowed_by_field),
    help("rename either the field or the method to export both"),
    severity(Warning)
)]
struct MethodShadowedByField {
    fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label("shadowed instance method")]
    span: Option<SourceSpan>,
}

#[cfg(test)]
mod tests {
    use crate::{compiler::check::tests::diagnostic_codes, diagnostic_test_db as test_db};

    #[test]
    fn warns_when_struct_field_shadows_instance_method() {
        let db = test_db! {
            #[derive(Clone)]
            pub struct Example {
                pub value: i32,
            }

            impl Example {
                pub fn value(&self) -> i32 {
                    self.value
                }
            }
        };

        assert_eq!(
            diagnostic_codes(&db),
            ["rust_curse::method_shadowed_by_field"]
        );
    }

    #[test]
    fn warns_once_when_enum_fields_shadow_a_method() {
        let db = test_db! {
            #[derive(Clone)]
            pub enum Example {
                First { value: i32 },
                Second { value: i32 },
            }

            impl Example {
                pub fn value(&self) -> i32 {
                    0
                }
            }
        };

        assert_eq!(
            diagnostic_codes(&db),
            ["rust_curse::method_shadowed_by_field"]
        );
    }

    #[test]
    fn static_method_does_not_shadow_instance_field() {
        let db = test_db! {
            #[derive(Clone)]
            pub struct Example {
                pub value: i32,
            }

            impl Example {
                pub fn value() -> i32 {
                    0
                }
            }
        };

        assert!(diagnostic_codes(&db).is_empty());
    }
}
