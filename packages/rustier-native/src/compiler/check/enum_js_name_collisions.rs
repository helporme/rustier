use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::rust::EnumType,
};

#[salsa::tracked]
impl<'db> EnumType<'db> {
    #[salsa::tracked]
    pub(super) fn check_js_name_collisions(self, db: &'db dyn Db) {
        let facade = self.interface(db).facade(db);
        let discriminator = facade.discriminator(db);

        if discriminator.name(db) != "$type" {
            return;
        }

        let source_span = self.def(db).span(db);
        let source_code: DiagSource = source_span
            .map(|span| span.src(db))
            .map(Into::into)
            .unwrap_or_default();
        let span = source_span.map(|span| (span.offset, span.length).into());

        EnumTypeFieldCollision { source_code, span }.accumulate(db);
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error(
    "enum field `type` conflicts with the generated JS discriminant. The discriminant is renamed to `$type`."
)]
#[diagnostic(
    code(rustier::enum_type_field_collision),
    help("use a different field name to avoid this collision"),
    severity(Warning)
)]
pub struct EnumTypeFieldCollision {
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<SourceSpan>,
}

#[cfg(test)]
mod tests {
    use crate::{compiler::check::tests::diagnostic_codes, diagnostic_test_db as test_db};

    #[test]
    fn warns_when_type_field_conflicts_with_discriminant() {
        let db = test_db! {
            #[derive(Clone)]
            pub enum Example {
                Value { r#type: String },
            }
        };

        assert_eq!(
            diagnostic_codes(&db),
            ["rustier::enum_type_field_collision"]
        );
    }

    #[test]
    fn accepts_variant_names_that_collide_when_camel_cased() {
        let db = test_db! {
            #[allow(non_camel_case_types)]
            #[derive(Clone)]
            pub enum Example {
                http {},
                Http,
            }
        };

        assert!(diagnostic_codes(&db).is_empty());
    }

    #[test]
    fn warns_only_for_type_field_when_variant_names_collide_when_camel_cased() {
        let db = test_db! {
            #[allow(non_camel_case_types)]
            #[derive(Clone)]
            pub enum Example {
                http { r#type: String },
                Http,
            }
        };

        assert_eq!(
            diagnostic_codes(&db),
            ["rustier::enum_type_field_collision"]
        );
    }

    #[test]
    fn accepts_distinct_facade_names() {
        let db = test_db! {
            #[derive(Clone)]
            pub enum Example {
                Http,
                HTTPRequest,
                Other { value: String },
            }
        };

        assert!(diagnostic_codes(&db).is_empty());
    }
}
