use std::collections::BTreeSet;

use miette::Diagnostic;
use thiserror::Error;

use crate::codegen::{
    db::Db,
    ir::span::{SourceCode, Span},
    mangle::to_camel_case,
    rustdoc::{
        ItemIdExt as _,
        mod_::{all_mod_ids, lower_mod_enums},
    },
};

/// Emits warnings for enum names that require collision-avoidance in the
/// JavaScript facade.
#[salsa::tracked(returns(ref))]
pub fn check_enum_js_name_collisions(db: &dyn Db) -> Vec<EnumJsNameCollision> {
    let mut diags = Vec::new();

    for mod_id in all_mod_ids(db) {
        for enum_ in lower_mod_enums(db, *mod_id) {
            let has_type_field_collision = enum_.js_tag_field_name(db) == "$type";
            let mut tags = BTreeSet::new();
            let has_variant_tag_collision = enum_
                .variants(db)
                .iter()
                .map(|variant| to_camel_case(variant.name(db)))
                .any(|tag| !tags.insert(tag));

            if !has_type_field_collision && !has_variant_tag_collision {
                continue;
            }

            let id = enum_.id(db).rustdoc_id(db);
            let item = id.item(db);
            let crate_ = id.crate_(db);
            let source_code = crate_.read_source_code_for_span_opt(item.span.as_ref());
            let span = crate_.read_span_opt(item.span.as_ref());

            if has_type_field_collision {
                diags.push(EnumJsNameCollision::TypeField {
                    source_code: source_code.clone(),
                    span,
                });
            }

            if has_variant_tag_collision {
                diags.push(EnumJsNameCollision::VariantTags { source_code, span });
            }
        }
    }

    diags
}

/// Warning emitted when enum names collide in the generated JavaScript API.
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
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
    },

    #[error(
        "enum variant names collide after conversion to lower camel case. The generated JS tags will preserve their Rust names"
    )]
    #[diagnostic(
        code(rust_curse::enum_variant_tag_collision),
        help("use a different field name to avoid this collision"),
        severity(Warning)
    )]
    VariantTags {
        #[source_code]
        source_code: SourceCode,
        #[label]
        span: Option<Span>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::test::test_db;

    fn diagnostic_codes(db: &dyn Db) -> Vec<String> {
        check_enum_js_name_collisions(db)
            .iter()
            .map(|diag| diag.code().unwrap().to_string())
            .collect()
    }

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
