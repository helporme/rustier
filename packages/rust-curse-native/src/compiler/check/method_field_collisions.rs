use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::{
        rust::{Enum, Field, FieldName, Struct},
        shim::ShimClass,
    },
};

#[salsa::tracked]
impl<'db> Struct<'db> {
    #[salsa::tracked]
    pub(super) fn check_method_field_collisions(self, db: &'db dyn Db) {
        check_method_field_collisions(db, self.shim(db), self.fields(db));
    }
}

#[salsa::tracked]
impl<'db> Enum<'db> {
    #[salsa::tracked]
    pub(super) fn check_method_field_collisions(self, db: &'db dyn Db) {
        check_method_field_collisions(db, self.shim(db), self.fields(db));
    }
}

fn check_method_field_collisions<'db>(
    db: &'db dyn Db,
    class: ShimClass<'db>,
    fields: impl Iterator<Item = Field<'db>>,
) {
    let methods = class.methods(db);

    for field in fields {
        let FieldName::Name(field_name) = field.name(db) else {
            continue;
        };
        let is_shadowed = methods.iter().any(|method| {
            method.src(db).receiver(db).is_some() && method.src(db).name(db) == field_name
        });
        if !is_shadowed {
            continue;
        }

        let source_span = field.span(db);
        let source_code = source_span
            .map(|span| span.src(db))
            .map(Into::into)
            .unwrap_or_default();
        let span = source_span.map(|span| (span.offset, span.length).into());

        FieldShadowedByMethod {
            field_name: field_name.clone(),
            source_code,
            span,
        }
        .accumulate(db);
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error(
    "the field `{field_name}` is not exported to JS because it is shadowed by a method with the same name"
)]
#[diagnostic(
    code(rust_curse::field_shadowed_by_method),
    help("rename either the field or the method to export both"),
    severity(Warning)
)]
struct FieldShadowedByMethod {
    field_name: String,
    #[source_code]
    source_code: DiagSource,
    #[label("shadowed field")]
    span: Option<SourceSpan>,
}

#[cfg(test)]
mod tests {
    use crate::{compiler::check::tests::diagnostic_codes, test_db};

    #[test]
    fn warns_when_instance_method_shadows_struct_field() {
        let db = test_db! {
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
            ["rust_curse::field_shadowed_by_method"]
        );
    }

    #[test]
    fn warns_for_each_shadowed_enum_variant_field() {
        let db = test_db! {
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
            [
                "rust_curse::field_shadowed_by_method",
                "rust_curse::field_shadowed_by_method",
            ]
        );
    }

    #[test]
    fn static_method_does_not_shadow_instance_field() {
        let db = test_db! {
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
