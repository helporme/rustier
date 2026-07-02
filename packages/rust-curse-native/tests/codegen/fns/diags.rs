use miette::Diagnostic;
use rust_curse::codegen::diag::Diag;

use crate::utils::*;

fn assert_skipped_fn(diags: &[Diag], expected_causes: &[&str]) {
    assert_eq!(diags.len(), 1);
    assert_eq!(diag_code(diags[0].inner.as_ref()), "rust_curse::skipped_fn");

    let actual_causes = related(diags[0].inner.as_ref())
        .into_iter()
        .map(diag_code)
        .collect::<Vec<_>>();
    assert_eq!(actual_causes, expected_causes);
}

fn assert_unsupported_primitive_param(diag: &dyn Diagnostic) {
    assert_eq!(diag_code(diag), "rust_curse::unsupported_param");

    let causes = related(diag);
    assert_eq!(causes.len(), 1);
    assert_eq!(diag_code(causes[0]), "rust_curse::unsupported_primitive");
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
fn unsupported_async_fn() {
    let (diags, _) = diag! {
        pub async fn foo() { todo!() }
    };

    assert_skipped_fn(&diags, &["rust_curse::unsupported_async_fn"]);
}

#[test]
fn unsupported_unsafe_fn() {
    let (diags, _) = diag! {
        pub unsafe fn foo() { todo!() }
    };

    assert_skipped_fn(&diags, &["rust_curse::unsupported_unsafe_fn"]);
}

#[test]
fn unsupported_abi_fn() {
    let (diags, _) = diag! {
        pub extern "C" fn foo() { todo!() }
    };

    assert_skipped_fn(&diags, &["rust_curse::unsupported_abi"]);
}

#[test]
fn unsupported_variadic_fn() {
    let (diags, _) = diag! {
        #![feature(c_variadic)]

        pub unsafe extern "C" fn foo(_: i32, _: ...) { todo!() }
    };

    assert_skipped_fn(
        &diags,
        &[
            "rust_curse::unsupported_unsafe_fn",
            "rust_curse::unsupported_abi",
            "rust_curse::unsupported_variadic_fn",
        ],
    );
}

#[test]
fn unsupported_generic_fn() {
    let (diags, _) = diag! {
        pub fn foo<T>(_: T) { todo!() }
    };

    assert_skipped_fn(
        &diags,
        &[
            "rust_curse::unsupported_generic_fn",
            "rust_curse::unsupported_param",
        ],
    );
}

#[test]
fn unsupported_param() {
    let (diags, _) = diag! {
        pub fn foo(_: u64) { todo!() }
    };

    assert_skipped_fn(&diags, &["rust_curse::unsupported_param"]);
    let causes = related(diags[0].inner.as_ref());
    assert_unsupported_primitive_param(causes[0]);
}

#[test]
fn unsupported_primitive_params() {
    let (diags, _) = diag! {
        pub fn foo(_: i128, _: u128) { todo!() }
    };

    assert_skipped_fn(
        &diags,
        &[
            "rust_curse::unsupported_param",
            "rust_curse::unsupported_param",
        ],
    );
    for cause in related(diags[0].inner.as_ref()) {
        assert_unsupported_primitive_param(cause);
    }
}

#[test]
fn lifetimes() {
    let (diags, _) = diag! {
        pub fn foo<'a>(value: &'a i32, b: &'a i32) -> i32 { *value }
    };

    assert_skipped_fn(
        &diags,
        &[
            "rust_curse::unsupported_generic_fn",
            "rust_curse::unsupported_param",
            "rust_curse::unsupported_param",
        ],
    );
}

#[test]
fn borrowed_return() {
    let (diags, _) = diag! {
        pub fn foo(value: &i32) -> &i32 { value }
    };

    assert_skipped_fn(&diags, &["rust_curse::borrowed_return"]);
}

#[test]
fn primitive_mut_ref_ignored() {
    let (diags, _) = diag! {
        pub fn foo(value: &mut i32) { *value += 1; }
    };

    assert_eq!(diags.len(), 1);
    assert_eq!(
        diag_code(diags[0].inner.as_ref()),
        "rust_curse::primitive_mut_ref_ignored"
    );
    assert!(related(diags[0].inner.as_ref()).is_empty());
}

#[test]
fn f32_param_precision_loss() {
    let (diags, _) = diag! {
        pub fn foo(value: f32) { let _ = value; }
    };

    assert_eq!(diags.len(), 1);
    assert_eq!(
        diag_code(diags[0].inner.as_ref()),
        "rust_curse::f32_precision_loss"
    );
    assert!(related(diags[0].inner.as_ref()).is_empty());
}

#[test]
fn f32_return_precision_loss() {
    let (diags, _) = diag! {
        pub fn foo() -> f32 { 1.25 }
    };

    assert_eq!(diags.len(), 1);
    assert_eq!(
        diag_code(diags[0].inner.as_ref()),
        "rust_curse::f32_precision_loss"
    );
    assert!(related(diags[0].inner.as_ref()).is_empty());
}

#[test]
fn f32_param_and_return_emit_one_precision_loss_diag() {
    let (diags, _) = diag! {
        pub fn foo(value: f32) -> f32 { value }
    };

    assert_eq!(diags.len(), 1);
    assert_eq!(
        diag_code(diags[0].inner.as_ref()),
        "rust_curse::f32_precision_loss"
    );
    assert!(related(diags[0].inner.as_ref()).is_empty());
}

#[test]
fn snap_chained_diags() {
    assert_snap! {
        @gen: diags;
        @category: diags;

        pub struct MyStruct {
            pub field: i32,
        }

        pub unsafe fn foo(a: MyStruct, b: u64, c: &&mut usize) { }
    }
}

#[test]
fn snap_macro_diags() {
    assert_snap! {
        @gen: diags;
        @category: diags;

        macro_rules! foo_fn {
            () => {
                pub unsafe fn foo(a: MyStruct, b: u64, c: &&mut usize) { }
            }
        }

        pub struct MyStruct {
            pub field: i32,
        }

        foo_fn!();
    }
}
