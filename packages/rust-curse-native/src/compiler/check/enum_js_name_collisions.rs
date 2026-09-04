use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::{facade::FacadeClass, rust::Enum},
    mangle::to_camel_case,
};

#[salsa::tracked]
impl<'db> Enum<'db> {
    #[salsa::tracked]
    pub(super) fn check_js_name_collisions(self, db: &'db dyn Db) {
        let facade = self.shim(db).facade(db);
        let FacadeClass::Discriminated(facade) = facade else {
            unreachable!();
        };
        let discriminator = facade.discriminator(db);

        let has_type_field_collision = discriminator.name(db) == "$type";
        let expected_variant_names = self
            .variants(db)
            .iter()
            .map(|variant| to_camel_case(variant.name(db)))
            .collect::<Vec<_>>();
        let has_variant_tag_collision = discriminator.variant_names(db) != &expected_variant_names;

        if !has_type_field_collision && !has_variant_tag_collision {
            return;
        }

        let source_span = self.span(db);
        let source_code: DiagSource = source_span
            .map(|span| span.src(db))
            .map(Into::into)
            .unwrap_or_default();
        let span = source_span.map(|span| (span.offset, span.length).into());

        if has_type_field_collision {
            EnumJsNameCollision::TypeField {
                source_code: source_code.clone(),
                span,
            }
            .accumulate(db);
        }

        if has_variant_tag_collision {
            EnumJsNameCollision::VariantTags { source_code, span }.accumulate(db);
        }
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum EnumJsNameCollision {
    #[error(
        "enum field `type` conflicts with the generated JS discriminant. The discriminant is renamed to `$type`."
    )]
    #[diagnostic(
        code(rust_curse::enum_type_field_collision),
        help("use a different field name to avoid this collision"),
        severity(Warning)
    )]
    TypeField {
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<SourceSpan>,
    },

    #[error(
        "enum variant names collide after conversion to lower camel case. The generated JS tags will preserve their Rust names"
    )]
    #[diagnostic(
        code(rust_curse::enum_variant_tag_collision),
        help("use a different variant name to avoid this collision"),
        severity(Warning)
    )]
    VariantTags {
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<SourceSpan>,
    },
}

#[cfg(test)]
mod tests {
    use crate::{compiler::check::tests::diagnostic_codes, test_db};

    #[test]
    fn warns_when_type_field_conflicts_with_discriminant() {
        let db = test_db! {
            pub enum Example {
                Value { r#type: String },
            }
        };

        assert_eq!(
            diagnostic_codes(&db),
            ["rust_curse::enum_type_field_collision"]
        );
    }

    #[test]
    fn warns_when_camel_cased_variant_tags_collide() {
        let db = test_db! {
            #[allow(non_camel_case_types)]
            pub enum Example {
                http {},
                Http,
            }
        };

        assert_eq!(
            diagnostic_codes(&db),
            ["rust_curse::enum_variant_tag_collision"]
        );
    }

    #[test]
    fn emits_both_collision_diagnostics_in_stable_order() {
        let db = test_db! {
            #[allow(non_camel_case_types)]
            pub enum Example {
                http { r#type: String },
                Http,
            }
        };

        assert_eq!(
            diagnostic_codes(&db),
            [
                "rust_curse::enum_type_field_collision",
                "rust_curse::enum_variant_tag_collision",
            ]
        );
    }

    #[test]
    fn accepts_distinct_facade_names() {
        let db = test_db! {
            pub enum Example {
                Http,
                HTTPRequest,
                Other { value: String },
            }
        };

        assert!(diagnostic_codes(&db).is_empty());
    }
}
