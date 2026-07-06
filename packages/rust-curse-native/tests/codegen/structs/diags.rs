use miette::Diagnostic;
use rust_curse::codegen::diag::Diag;

use crate::utils::*;

fn assert_skipped_struct(diags: &[Diag], expected_causes: &[&str]) {
    assert_eq!(diags.len(), 1);
    assert_eq!(
        diag_code(diags[0].inner.as_ref()),
        "rust_curse::skipped_struct"
    );

    let actual_causes = related(diags[0].inner.as_ref())
        .into_iter()
        .map(diag_code)
        .collect::<Vec<_>>();
    assert_eq!(actual_causes, expected_causes);
}

fn assert_field_skipped_struct(diags: &[Diag], expected_field_causes: &[&[&str]]) {
    assert_eq!(diags.len(), 1);
    assert_eq!(
        diag_code(diags[0].inner.as_ref()),
        "rust_curse::skipped_struct"
    );

    let fields = related(diags[0].inner.as_ref());
    assert_eq!(fields.len(), expected_field_causes.len());

    for (field, expected_causes) in fields.into_iter().zip(expected_field_causes) {
        assert_eq!(diag_code(field), "rust_curse::skipped_field");

        let actual_causes = related(field)
            .into_iter()
            .map(diag_code)
            .collect::<Vec<_>>();
        assert_eq!(actual_causes, *expected_causes);
    }
}

fn diag_code(diag: &dyn Diagnostic) -> String {
    diag.code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "<missing code>".to_string())
}

fn related(diag: &dyn Diagnostic) -> Vec<&dyn Diagnostic> {
    diag.related()
        .map(|related| related.collect())
        .unwrap_or_default()
}

#[test]
fn unsupported_generic_struct() {
    let (diags, _) = diag! {
        pub struct PlainStruct<T> {
            pub value: T,
        }
    };

    assert_skipped_struct(&diags, &["rust_curse::unsupported_generic_struct"]);
}

#[test]
fn plain_field_with_unsupported_primitive() {
    let (diags, _) = diag! {
        pub struct PlainStruct {
            pub value: u64,
        }
    };

    assert_field_skipped_struct(&diags, &[&["rust_curse::unsupported_primitive"]]);
}

#[test]
fn tuple_field_with_unsupported_primitive() {
    let (diags, _) = diag! {
        pub struct TupleStruct(pub u64);
    };

    assert_field_skipped_struct(&diags, &[&["rust_curse::unsupported_primitive"]]);
}

#[test]
fn plain_field_with_unsupported_type() {
    let (diags, _) = diag! {
        pub struct PlainStruct {
            pub value: Vec<i32>,
        }
    };

    assert_field_skipped_struct(&diags, &[&["rust_curse::unsupported_type"]]);
}

#[test]
fn tuple_field_with_unsupported_type() {
    let (diags, _) = diag! {
        pub struct TupleStruct(pub Vec<i32>);
    };

    assert_field_skipped_struct(&diags, &[&["rust_curse::unsupported_type"]]);
}

#[test]
fn plain_field_with_unsupported_lifetime_ref() {
    let (diags, _) = diag! {
        pub struct PlainStruct {
            pub value: &'static i32,
        }
    };

    assert_field_skipped_struct(&diags, &[&["rust_curse::unsupported_lifetime_ref"]]);
}

#[test]
fn tuple_field_with_unsupported_lifetime_ref() {
    let (diags, _) = diag! {
        pub struct TupleStruct(pub &'static i32);
    };

    assert_field_skipped_struct(&diags, &[&["rust_curse::unsupported_lifetime_ref"]]);
}

#[test]
fn plain_field_with_nested_unsupported_lifetime_refs() {
    let (diags, _) = diag! {
        pub struct PlainStruct {
            pub value: &'static &'static i32,
        }
    };

    assert_field_skipped_struct(
        &diags,
        &[&[
            "rust_curse::unsupported_lifetime_ref",
            "rust_curse::unsupported_lifetime_ref",
        ]],
    );
}

#[test]
fn snap_plain_field_with_unsupported_primitive() {
    assert_snap! {
        @gen: diags;
        @category: diags;

        pub struct PlainStruct {
            pub supported: i32,
            pub unsupported: u64,
        }
    }
}
